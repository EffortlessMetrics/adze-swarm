import pathlib, subprocess, os, json, hashlib, shutil, tempfile, argparse
args_parser=argparse.ArgumentParser(description="Real-Cargo optional Rust failure/C fallback invalidation control")
args_parser.add_argument('--target-dir', type=pathlib.Path, help="Explicitly owned reusable target; default is inside unique proof directory")
args_parser.add_argument('--expect-stale', action='store_true', help="Qualify the original stale-artifact counterexample")
args=args_parser.parse_args()
P=pathlib.Path(tempfile.mkdtemp(prefix='adze-optional-watch-proof-'))
project=P/'project'; target=args.target_dir.resolve() if args.target_dir else P/'target'
(project/'grammar').mkdir(parents=True);(project/'src').mkdir()
tool=pathlib.Path(__file__).resolve().parents[3]
print('proof='+str(P),flush=True)
(project/'Cargo.toml').write_text('[package]\nname="optional-watch-probe"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[build-dependencies]\nadze-tool={path='+json.dumps(str(tool))+'}\n')
(project/'src/lib.rs').write_text('pub fn consumer_is_empty() {}\n')
(project/'grammar/root.rs').write_text('mod words;\n')
child=project/'grammar/words.rs'
child.write_text('#[adze::grammar("probe")]\npub mod grammar { #[adze::language] pub struct Word { #[adze::leaf(text="hello")] value: (), } }\n')
build='''use std::{env,fs,path::Path};
fn main() {
 let counter=env::var_os("ADZE_PROBE_COUNTER").unwrap();
 let old=fs::read_to_string(&counter).unwrap_or_else(|_| "0".into()).parse::<u32>().unwrap();
 fs::write(counter,(old+1).to_string()).unwrap();
 BLOCKER
 adze_tool::build_parsers(Path::new("grammar/root.rs"));
}
'''
block='let out=std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap()); let path=out.join("grammar_probe/parser_probe.rs"); if path.is_file() { fs::remove_file(&path).unwrap(); } fs::create_dir_all(path).unwrap();'
(project/'build.rs').write_text(build.replace('BLOCKER',block))
env=os.environ.copy(); env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_NET_OFFLINE='true',ADZE_PROBE_COUNTER=str(P/'counter'))
for key in ['ADZE_USE_PURE_RUST','ADZE_EMIT_ARTIFACTS','RUST_LOG','ADZE_DEBUG_FILE','ADZE_REQUIRE_TS_CLI']: env.pop(key,None)
r=subprocess.run(['cargo','generate-lockfile','--offline'],cwd=project,env=env,capture_output=True,timeout=180)
(P/'lock.stdout').write_bytes(r.stdout);(P/'lock.stderr').write_bytes(r.stderr)
if r.returncode: raise SystemExit(r.returncode)
receipt={'tool':str(tool),'expect_stale':args.expect_stale,'driver_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'steps':[]}
def run(name):
 r=subprocess.run(['cargo','build','--offline','--locked','--message-format=json'],cwd=project,env=env,capture_output=True,timeout=180)
 (P/(name+'.stdout')).write_bytes(r.stdout);(P/(name+'.stderr')).write_bytes(r.stderr)
 step={'name':name,'exit':r.returncode,'counter':(P/'counter').read_text() if (P/'counter').exists() else None,'files':{}}
 messages=[json.loads(line) for line in r.stdout.splitlines() if line.startswith(b'{')]
 active=[pathlib.Path(row['out_dir']) for row in messages if row.get('reason')=='build-script-executed' and 'optional-watch-probe' in row.get('package_id','')]
 if r.returncode:
  receipt['steps'].append(step);(P/'receipt.json').write_text(json.dumps(receipt,indent=2));raise SystemExit(r.returncode)
 assert len(active)==1, active
 out=active[0]
 for f in [out.parent/'output',out/'last_grammar.json',out/'libprobe.a']:
   data=f.read_bytes(); dest=P/name/f.relative_to(target);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
   step['files'][str(f.relative_to(target))]={'sha256':hashlib.sha256(data).hexdigest(),'snapshot':str(dest)}
 receipt['steps'].append(step);(P/'receipt.json').write_text(json.dumps(receipt,indent=2));print(json.dumps(step),flush=True)
 if r.returncode: raise SystemExit(r.returncode)
run('initial_blocked_optional')
run('unchanged_blocked_optional')
child.write_text(child.read_text().replace('hello','goodbye'))
run('child_only_changed')
for f in [pathlib.Path(next(iter(receipt['steps'][-1]['files']))).parent/'out/grammar_probe/parser_probe.rs']:
 f=target/f
 if f.is_dir(): shutil.rmtree(f)
(project/'build.rs').write_text(build.replace('BLOCKER','// legitimate blocker removal'))
run('blocker_removed_build_script_changed')

steps=receipt['steps']
expected=['1','1','1','2'] if args.expect_stale else ['1','1','2','3']
assert [s['counter'] for s in steps]==expected, receipt
for index,step in enumerate(steps):
    grammar=pathlib.Path(next(v['snapshot'] for k,v in step['files'].items() if k.endswith('last_grammar.json'))).read_text()
    expect_goodbye=index>=3 if args.expect_stale else index>=2
    assert ('goodbye' in grammar)==expect_goodbye, (step,grammar)
    assert ('hello' in grammar)!=expect_goodbye, (step,grammar)
    output=pathlib.Path(next(v['snapshot'] for k,v in step['files'].items() if k.endswith('/output'))).read_text()
    assert ('Failed to build optional pure-Rust parser modules' in output)==(index<3), output
    assert ('rerun-if-changed=' in output)==(not args.expect_stale or index==3), output
archive=lambda s: next(v['sha256'] for k,v in s['files'].items() if k.endswith('libprobe.a'))
assert archive(steps[0])==archive(steps[1])
assert (archive(steps[1])==archive(steps[2]))==args.expect_stale
assert archive(steps[0])!=archive(steps[3])
parser_modules=[target/pathlib.Path(next(iter(steps[-1]['files']))).parent/'out/grammar_probe/parser_probe.rs']
assert len(parser_modules)==1 and parser_modules[0].is_file()
shutil.copyfile(parser_modules[0], P/'recovered-parser_probe.rs')
receipt['result']='confirmed_stale' if args.expect_stale else 'pass'
(P/'receipt.json').write_text(json.dumps(receipt,indent=2))
print('receipt='+str(P/'receipt.json'),flush=True)
