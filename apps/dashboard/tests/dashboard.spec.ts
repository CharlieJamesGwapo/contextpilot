import { test, expect } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtemp, readFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

let engine:ChildProcess|undefined;
let dataDir='';
let baseURL='';
let token='';
async function cleanup(){
  if(engine&&engine.exitCode===null&&engine.signalCode===null){
    engine.kill('SIGINT');
    const stopped=await Promise.race([new Promise<boolean>(resolve=>engine!.once('exit',()=>resolve(true))),delay(5000).then(()=>false)]);
    if(!stopped){engine.kill('SIGKILL');await new Promise<void>(resolve=>engine!.once('exit',()=>resolve()))}
  }
  if(dataDir)await rm(dataDir,{recursive:true,force:true});
  token='';
}
test.beforeAll(async()=>{
  const root=path.resolve(__dirname,'..');
  const binary=path.resolve(process.env.CONTEXTPILOT_BINARY||path.join(root,'../../target/debug/contextpilot'));
  await access(binary).catch(()=>{throw new Error('Build the Rust debug binary with cargo build first, or set CONTEXTPILOT_BINARY.')});
  dataDir=await mkdtemp(path.join(tmpdir(),'contextpilot-browser-'));
  let stderr='';let startupError='';
  engine=spawn(binary,['--data-dir',dataDir,'dashboard','--port','0','--assets',path.join(root,'out')],{stdio:['ignore','ignore','pipe'],env:{...process.env,OPENAI_API_KEY:undefined,ANTHROPIC_API_KEY:undefined}});
  engine.on('error',()=>{startupError='Rust dashboard process failed to start.'});
  engine.stderr!.on('data',(chunk:Buffer)=>{stderr+=chunk.toString()});
  try{
    const deadline=Date.now()+15000;
    while(Date.now()<deadline){
      if(startupError)throw new Error(startupError);
      const match=stderr.match(/http:\/\/127\.0\.0\.1:(\d+)/);
      if(match){
        baseURL=`http://127.0.0.1:${match[1]}`;
        token=(await readFile(path.join(dataDir,`dashboard-${match[1]}.token`),'utf8')).trim();
        for(let i=0;i<50;i++){
          try{if((await fetch(baseURL)).ok)return}catch{/* Server may still be binding. */}
          await delay(50);
        }
        throw new Error('Dashboard did not become ready.');
      }
      if(engine.exitCode!==null)throw new Error('Rust dashboard exited during startup.');
      await delay(50);
    }
    throw new Error('Timed out waiting for Rust dashboard.');
  }catch(e){await cleanup();throw e}
});
test.afterAll(cleanup);

test('local context workflow persists and restores without external requests',async({page})=>{
  const browserErrors:string[]=[];const failedResponses:string[]=[];let externalRequest=false;let missingAuth=false;
  page.on('pageerror',e=>browserErrors.push(e.message));
  page.on('response',r=>{if(r.status()>=400)failedResponses.push(`${r.status()} ${new URL(r.url()).pathname}`)});
  page.on('request',r=>{
    const url=new URL(r.url());
    if(url.origin!==baseURL)externalRequest=true;
    if(url.pathname.startsWith('/api/v1/')&&r.headers().authorization!==`Bearer ${token}`)missingAuth=true;
  });
  await page.goto(baseURL);
  await expect(page.getByRole('heading',{name:'Open your workspace'})).toBeVisible();
  await expect(page.locator('.brand')).toContainText('ContextPilot');
  await expect(page.getByRole('heading',{name:'Not available',exact:true})).toBeVisible();
  await expect(page.getByText('It is not a paid AI API key.',{exact:false})).toBeVisible();
  await page.getByText('Where do I find my connection code?',{exact:true}).click();
  await expect(page.getByText('Open the terminal window running ContextPilot.',{exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Your saved work will appear here'})).toBeVisible();
  // Secrets never appear in test titles, logs, traces, screenshots, URLs or assertions.
  await page.getByLabel('Connection code',{exact:true}).fill(token);
  await page.getByRole('button',{name:'Connect',exact:true}).click();
  await expect(page.getByText('Connected to this computer')).toBeVisible();
  const navigate=async(name:string)=>{await page.locator('nav button').filter({hasText:name}).click()};
  await navigate('Prompts');
  const source='Background: Build a recoverable local workflow.\nBackground: Build a recoverable local workflow.\nExample: Preserve useful context.\nExample: Preserve useful context.\nConstraint: Never send prompts externally without consent.\nRequirement: Preserve all explicit requirements.';
  await page.getByLabel('Your prompt',{exact:true}).fill(source);
  await page.getByRole('button',{name:'Optimize',exact:true}).click();
  await expect(page.getByText('Preservation checks passed')).toBeVisible();
  const output=page.getByLabel('Editable optimized prompt');
  const optimized=await output.inputValue();
  expect(optimized).toContain('Constraint: Never send prompts externally without consent.');
  expect(optimized).toContain('Requirement: Preserve all explicit requirements.');
  expect(optimized.length).toBeLessThan(source.length);
  await page.getByText('Optional: ask an AI service',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Run deep optimization'})).toBeDisabled();
  await output.fill('Manually reviewed prompt');
  await page.context().grantPermissions(['clipboard-read','clipboard-write']);
  await page.getByRole('button',{name:'Copy prompt',exact:true}).click();
  expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe('Manually reviewed prompt');
  const promptDownload=page.waitForEvent('download');
  await page.getByRole('button',{name:'Download',exact:true}).click();
  const promptFile=await (await promptDownload).path();
  expect(await readFile(promptFile!,'utf8')).toBe('Manually reviewed prompt');
  await page.getByRole('button',{name:'Restore original'}).click();
  await expect(output).toHaveValue(source);

  await navigate('Sessions');
  await page.getByRole('button',{name:'New session'}).click();
  await page.locator('[name=objective]').fill('Keep the local recovery path complete');
  await page.locator('[name=requirements]').fill('Preserve requirements\nSupport a continuation checkpoint');
  await page.locator('[name=constraints]').fill('External calls require explicit consent');
  await page.locator('[name=issues]').fill('Verify checkpoint restoration');
  await page.locator('[name=steps]').fill('Restore and verify requirements');
  await page.locator('[name=limit]').fill('10000');
  await page.getByRole('button',{name:'Create session',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Keep the local recovery path complete',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Session capacity'})).toBeVisible();
  for(let i=0;i<2;i++){
    await page.getByLabel('Add context',{exact:true}).fill('Decision: Keep the complete recovery path.');
    await page.getByRole('button',{name:'Add context',exact:true}).click();
    await expect(page.getByRole('heading',{name:`Context messages · ${i+1}`,exact:true})).toBeVisible();
  }
  await page.getByRole('button',{name:'Preview checkpoint'}).click();
  await expect(page.getByRole('heading',{name:'Checkpoint preview',exact:true})).toBeVisible();
  await expect(page.locator('.preview .output')).toContainText('Preserve requirements');
  await page.getByRole('button',{name:'Save checkpoint',exact:true}).click();
  await expect(page.getByText('Checkpoint saved.',{exact:true})).toBeVisible();
  page.once('dialog',dialog=>dialog.accept());
  await page.locator('.checkpoint').first().getByRole('button',{name:'Restore',exact:true}).click();
  await expect(page.getByText('Checkpoint restored into a session.',{exact:true})).toBeVisible();
  await page.getByText('Initial brief · requirements, constraints and next steps',{exact:true}).click();
  await expect(page.locator('details').filter({has:page.locator('summary').filter({hasText:'Initial brief · requirements, constraints and next steps'})}).getByText('Preserve requirements',{exact:true})).toBeVisible();
  await expect(page.locator('details').filter({has:page.locator('summary').filter({hasText:'Initial brief · requirements, constraints and next steps'})}).getByText('External calls require explicit consent',{exact:true})).toBeVisible();

  await navigate('Settings');
  await page.getByLabel('History retention (days)',{exact:true}).fill('45');
  await page.getByLabel('Automatically compact safe duplicates in managed sessions').check();
  await page.getByRole('button',{name:'Save settings'}).click();
  await expect(page.getByText('Settings saved.',{exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByText('Connected to this computer')).toBeVisible();
  await navigate('Settings');
  await expect(page.getByLabel('History retention (days)',{exact:true})).toHaveValue('45');
  await expect(page.getByLabel('Automatically compact safe duplicates in managed sessions')).toBeChecked();
  await navigate('Sessions');
  await page.locator('.session-item').first().click();
  await page.getByText('Initial brief · requirements, constraints and next steps',{exact:true}).click();
  await expect(page.locator('details').filter({has:page.locator('summary').filter({hasText:'Initial brief · requirements, constraints and next steps'})}).getByText('Preserve requirements',{exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Session capacity'})).toBeVisible();
  await page.setViewportSize({width:390,height:844});
  for(const name of ['Home','Projects','Prompts','Sessions','Usage','Connections','Settings']){
    await navigate(name);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),`No mobile overflow on ${name}`).toBe(true);
  }
  expect(externalRequest).toBe(false);
  expect(missingAuth).toBe(false);
  expect(browserErrors).toEqual([]);
  expect(failedResponses).toEqual([]);
});


test('connection help explains local access and invalid codes have a useful recovery message',async({page})=>{
  await page.goto(baseURL);
  await page.getByLabel('Connection code',{exact:true}).fill('invalid-local-test-code');
  await page.getByRole('button',{name:'Connect',exact:true}).click();
  await expect(page.locator('.alert.error[role=alert]')).toContainText('Copy the current code from the ContextPilot terminal and try again.');
  await expect(page.getByRole('heading',{name:'Open your workspace',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Not available',exact:true})).toBeVisible();
});
