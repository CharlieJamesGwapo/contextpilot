#!/usr/bin/env python3
"""Real subprocess acceptance; no provider calls or writes outside a temp directory."""
import json, pathlib, subprocess, sys, tempfile
binary = pathlib.Path(sys.argv[1] if len(sys.argv)>1 else 'target/debug/contextpilot').resolve()
with tempfile.TemporaryDirectory(prefix='contextpilot-acceptance-') as temp:
    root=pathlib.Path(temp)
    def run(*args):
        p=subprocess.run([str(binary),'--data-dir',str(root/'data'),*args],cwd=root,text=True,capture_output=True)
        assert p.returncode==0, (args,p.stderr)
        return json.loads(p.stdout)
    run('doctor')
    run('init',str(root))
    prompt=root/'task.md'; prompt.write_text('Background example.\nBackground example.\nMust preserve src/auth.rs and run tests.\nNever deploy.')
    assert run('tokens',str(prompt))['measurement']=='estimated'
    optimized=run('optimize',str(prompt))
    assert optimized['tokens_reduced']>0
    assert 'Never deploy' in optimized['optimized']
    session=run('session','create','Fix login','--requirement','Test auth','--constraint','Never deploy','--issue','401','--next','Run auth tests','--limit','10000')
    sid=session['id']
    for _ in range(3): run('session','add',sid,'--text','Repeated background context.')
    budget=run('context','status','--session',sid);assert budget['limit']==10000
    preview=run('compact','--session',sid,'--preview');assert preview['tokens_reduced']>0
    checkpoint=run('compact','--session',sid,'--commit','--revision',str(preview['checkpoint']['source_revision']))['checkpoint']
    restored=run('checkpoint','restore',checkpoint['id'],'--yes');assert len(restored['messages'])==3
    status=run('status');assert status['sessions'][0]['constraints']==['Never deploy']
    assert len(status['optimizations'])==1
    run('backup',str(root/'backup.sqlite'))
    run('restore-backup',str(root/'backup.sqlite'),str(root/'recovered'))
    p=subprocess.run([str(binary),'--data-dir',str(root/'recovered'),'status'],capture_output=True,text=True)
    assert p.returncode==0
    assert json.loads(p.stdout)['sessions'][0]['id']==sid
    exported=root/'export.json';run('export',str(exported));assert exported.exists()
    run('session','delete',sid,'--yes');assert run('session','list')==[]
    repo=root/'repository';repo.mkdir()
    content='pub fn unique_symbol() {}\n'+'let repeated_line = 1;\n'*100
    for i in range(20): (repo/f'module{i:02}.rs').write_text(content)
    (repo/'excluded.rs').write_text(content)
    project=run('project','add',str(repo))
    patch=root/'project-policy.json';patch.write_text(json.dumps({'project_exclusions':['excluded.rs'],'profile':{'max_file_tokens':64,'max_files':2},'tokenizer_provider':'openai','tokenizer_model':'gpt-4o'}))
    run('project','config',project['id'],'--file',str(patch))
    directory=run('tokens',str(repo))
    assert directory['files_analyzed']==20 and directory['scope']=='eligible_full_files',directory
    single=run('tokens',str(repo/'module00.rs'),'--provider','openai','--model','gpt-4o')
    assert directory['tokens']==20*single['tokens'],directory
    assert directory['measurement']=='tokenized_text',directory
    indexed=run('project','index','--root',str(repo))
    assert len(indexed['files'])==20,indexed
    retrieved=run('retrieve','unique_symbol','--root',str(repo),'--max-files','8')
    assert len(retrieved)==2 and all(x['omitted'] and x['excerpt']=='' and x['note'] for x in retrieved),retrieved
print('PASS: init, estimate, optimize, sessions, budget, preview/commit, restore, restart, backup recovery, export, delete, full directory accounting, configured tokenizer/exclusions, bounded retrieval')
