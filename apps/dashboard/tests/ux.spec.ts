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

const sections=['Home','Projects','Prompts','Sessions','Usage','Connections','Settings'];
async function connect(page:import('@playwright/test').Page){
  await page.goto(baseURL);
  await page.getByLabel('Connection code',{exact:true}).fill(token);
  await page.getByRole('button',{name:'Connect',exact:true}).click();
  await expect(page.getByText('Connected to this computer')).toBeVisible();
}

test('responsive navigation, keyboard focus, theme and deep links stay local',async({page})=>{
  const errors:string[]=[];const external:string[]=[];
  page.on('pageerror',e=>errors.push(e.message));
  page.on('request',r=>{if(new URL(r.url()).origin!==baseURL)external.push(new URL(r.url()).origin)});
  await connect(page);
  const skip=page.getByRole('button',{name:'Skip tour',exact:true});
  await expect(skip).toBeVisible();await skip.click();
  await page.reload();await expect(skip).not.toBeVisible();
  await page.emulateMedia({reducedMotion:'reduce'});
  const theme=page.getByRole('button',{name:/Switch to (light|dark) theme/});
  const previousTheme=await theme.getAttribute('aria-label');
  await theme.click();await expect(theme).not.toHaveAttribute('aria-label',previousTheme!);
  for(let i=0;i<2;i++){
    for(const selector of ['.home-quick .secondary','.page-heading p','.brand-icon']){
      const colors=await page.locator(selector).first().evaluate(el=>{const s=getComputedStyle(el);return {text:s.color,background:el.matches('.page-heading p')?getComputedStyle(document.body).backgroundColor:s.backgroundColor}});
      const luminance=(color:string)=>{const rgb=color.match(/[\d.]+/g)!.slice(0,3).map(Number).map(v=>{v/=255;return v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4});return rgb[0]*0.2126+rgb[1]*0.7152+rgb[2]*0.0722};
      const values=[luminance(colors.text),luminance(colors.background)].sort((a,b)=>a-b);
      expect((values[1]+0.05)/(values[0]+0.05),`${selector} foreground contrast in both themes`).toBeGreaterThanOrEqual(selector==='.brand-icon'?3:4.5);
    }
    await theme.click();
  }
  const nextTheme=await theme.getAttribute('aria-label');
  await page.reload();await expect(theme).toHaveAttribute('aria-label',nextTheme!);
  for(const width of [320,360,390,430,768,1024,1280,1440]){
    await page.setViewportSize({width,height:900});
    for(const section of sections){
      const nav=page.getByRole('navigation',{name:'Main navigation'}).getByRole('button',{name:section,exact:false});
      await nav.click();
      await expect(page).toHaveURL(new RegExp(`#${section.toLowerCase()}$`));
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),`${width}px ${section} has no document overflow`).toBe(true);
    }
  }
  await page.getByRole('link',{name:'Skip to content',exact:true}).focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('main')).toBeFocused();
  await page.goto(`${baseURL}/#sessions`);
  await expect(page.getByRole('button',{name:'New session',exact:true})).toBeVisible();
  const home=page.getByRole('navigation',{name:'Main navigation'}).getByRole('button',{name:'Home',exact:true});
  await home.focus();await page.keyboard.press('Enter');await expect(page).toHaveURL(/#home$/);
  await page.keyboard.press('Tab');
  expect(await page.evaluate(()=>document.activeElement?.tagName==='BUTTON')).toBe(true);
  await page.goBack();await expect(page).toHaveURL(/#sessions$/);
  expect(errors).toEqual([]);expect(external).toEqual([]);
});

test('saved checkpoint can be inspected without changing session state',async({page})=>{
  const headers={Authorization:`Bearer ${token}`,'Content-Type':'application/json'};
  const post=async(endpoint:string,body:unknown)=>{
    const response=await fetch(`${baseURL}/api/v1/${endpoint}`,{method:'POST',headers,body:JSON.stringify(body)});
    expect(response.ok).toBe(true);return response.json();
  };
  const session=await post('sessions',{objective:'Inspect saved progress safely',requirements:['Preserve important instructions'],constraints:['Never push or deploy'],unresolved_issues:[],next_steps:['Continue with the next task'],context_limit:10000});
  await post(`sessions/${session.id}/messages`,{category:'verification',text:'Tests passed locally. Commit abc1234 is saved.'});
  const preview=await post('contexts/compact/preview',{session_id:session.id});
  await post('contexts/compact',{session_id:session.id,source_revision:preview.checkpoint.source_revision});
  const saved=(await (await fetch(`${baseURL}/api/v1/checkpoints`,{headers})).json()).find((c:{session_id:string})=>c.session_id===session.id);
  await connect(page);
  await expect(page.locator('.home-next')).toContainText('Tests passed locally. Commit abc1234 is saved.');
  const beforePreview=await (await fetch(`${baseURL}/api/v1/sessions/${session.id}`,{headers})).json();
  await page.locator('.home-current').getByRole('button',{name:'Preview checkpoint',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Checkpoint preview',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:session.objective,exact:true})).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`#sessions\\?session=${session.id}$`));
  expect(await (await fetch(`${baseURL}/api/v1/sessions/${session.id}`,{headers})).json()).toEqual(beforePreview);
  await page.goto(`${baseURL}/#sessions?session=${session.id}`);
  await expect(page.getByRole('heading',{name:session.objective,exact:true})).toBeVisible();
  const inspect=page.locator('summary').filter({hasText:'Inspect checkpoint'}).first();
  await expect(inspect).toBeVisible();await inspect.click();
  await expect(page.getByText('Never push or deploy',{exact:true}).last()).toBeVisible();
  page.once('dialog',dialog=>dialog.dismiss());
  await page.locator('.checkpoint').first().getByRole('button',{name:'Restore',exact:true}).click();
  const before=await (await fetch(`${baseURL}/api/v1/sessions/${session.id}`,{headers})).json();
  await page.reload();
  const after=await (await fetch(`${baseURL}/api/v1/sessions/${session.id}`,{headers})).json();
  expect(after).toEqual(before);
  await expect(page.locator('.checkpoint').first().getByRole('button',{name:'Export',exact:true})).toBeVisible();
  const downloaded=page.waitForEvent('download');
  await page.locator('.checkpoint').first().getByRole('button',{name:'Export JSON',exact:true}).click();
  const download=await downloaded;const file=await download.path();
  const exported=JSON.parse(await readFile(file!,'utf8'));
  expect(exported.session_id).toBe(session.id);expect(exported.constraints).toContain('Never push or deploy');
  expect(exported.integrity_hash).toBe(saved.integrity_hash);
  expect(exported.continuation).toContain('abc1234');
  await expect(page.locator('.checkpoint').first().getByRole('button',{name:'Restore',exact:true})).toBeVisible();
});
