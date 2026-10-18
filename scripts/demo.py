#!/usr/bin/env python3
"""Create isolated, explicitly synthetic demo state through the real CLI."""
import argparse,json,pathlib,shlex,subprocess
p=argparse.ArgumentParser(description='ContextPilot synthetic demo; never seeds provider usage')
p.add_argument('binary',type=pathlib.Path)
p.add_argument('--data-dir',type=pathlib.Path,default=pathlib.Path('.contextpilot-demo'))
a=p.parse_args();binary=a.binary.resolve();data=a.data_dir.resolve()
if not binary.is_file():p.error('Build the CLI first or provide its installed path')
if data.exists():p.error('Demo directory exists; choose a new directory')
data.mkdir(parents=True,mode=0o700)
def run(*args,input=None):
 r=subprocess.run([str(binary),'--data-dir',str(data),*args],input=input,capture_output=True,text=True,check=True)
 return json.loads(r.stdout)
s=run('session','create','[DEMO — synthetic] Fix a fictional login error','--requirement','Run the fictional regression test','--constraint','Never deploy this demo','--issue','Synthetic 401 error','--next','Inspect fictional src/auth.rs','--limit','16000')
for _ in range(3):run('session','add',s['id'],'--text','Background: fictional repository uses a local login form.')
run('optimize','--stdin',input='Background example.\nBackground example.\nMust preserve fictional src/auth.rs and run tests.\nNever deploy this demo.')
print('DEMO — synthetic prompts and task state. Counts come from the real approximate local engine. No provider usage or costs fabricated.')
print('Start the dashboard with:')
print(shlex.join([str(binary),'--data-dir',str(data),'dashboard']))
