import {test,expect,type Page} from '@playwright/test';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const objective='Browser-only synthetic task';
const note='Decision: Keep private context in this browser. Verification: offline checks passed.';
function watchPrivacy(page:Page,baseURL:string){
 const errors:string[]=[],network:string[]=[];
 page.on('pageerror',e=>errors.push(e.message));
 page.on('request',request=>{const url=new URL(request.url());if(url.origin!==new URL(baseURL).origin||url.pathname.startsWith('/api/')||request.headers().authorization)network.push(`${url.origin}${url.pathname}`)});
 return()=>{expect(errors).toEqual([]);expect(network).toEqual([])};
}
async function open(page:Page){
 page.setDefaultTimeout(10000);
 await page.addInitScript(()=>sessionStorage.setItem('contextpilot-token','synthetic-old-connection-code-not-a-real-secret'));
 await page.goto('/');
 await expect(page.getByText('Stored in this browser',{exact:true})).toBeVisible();
 await expect(page.getByLabel('Connection code',{exact:true})).toHaveCount(0);
 const skip=page.getByRole('button',{name:'Skip tour',exact:true});if(await skip.isVisible())await skip.click();
}
async function navigate(page:Page,name:string){await page.getByRole('navigation',{name:'Main navigation'}).getByRole('button',{name,exact:false}).click()}

test('browser workspace optimizes, saves and restores genuine checkpoints offline',async({page,baseURL,browser})=>{
 const verifyPrivacy=watchPrivacy(page,baseURL!);await open(page);
 await navigate(page,'Prompts');
 const prompt='Background: Build a recoverable local workflow.\nBackground: Build a recoverable local workflow.\nExample: Preserve useful context.\nExample: Preserve useful context.\nRequirement: Preserve all instructions.\nConstraint: Never upload source.';
 await page.getByLabel('Your prompt',{exact:true}).fill(prompt);await page.getByRole('button',{name:'Optimize',exact:true}).click();
 await expect(page.getByText('Preservation checks passed',{exact:true})).toBeVisible();
 const optimized=await page.getByLabel('Editable optimized prompt').inputValue();
 expect(optimized).toContain('Requirement: Preserve all instructions.');expect(optimized).toContain('Constraint: Never upload source.');expect(optimized.length).toBeLessThan(prompt.length);
 await navigate(page,'Sessions');await page.getByRole('button',{name:'New session',exact:true}).click();
 await page.locator('[name=objective]').fill(objective);await page.locator('[name=requirements]').fill('Preserve browser-only progress');await page.locator('[name=constraints]').fill('Never upload private notes');await page.locator('[name=steps]').fill('Export a verified checkpoint');await page.locator('[name=limit]').fill('10000');await page.getByRole('button',{name:'Create session',exact:true}).click();
 await expect(page.getByRole('heading',{name:objective,exact:true})).toBeVisible();
 await page.getByLabel('Add context',{exact:true}).fill(note);await page.getByRole('button',{name:'Add context',exact:true}).click();
 await page.getByRole('button',{name:'Preview checkpoint',exact:true}).click();await expect(page.getByRole('heading',{name:'Checkpoint preview',exact:true})).toBeVisible();
 await expect(page.locator('.preview .output')).toContainText('Never upload private notes');await page.getByRole('button',{name:'Save checkpoint',exact:true}).click();await expect(page.getByText('Checkpoint saved.',{exact:true})).toBeVisible();
 const checkpoint=page.locator('.checkpoint').first();await checkpoint.locator('summary').filter({hasText:'Inspect checkpoint'}).click();await expect(checkpoint).toContainText('Never upload private notes');
 const downloadReady=page.waitForEvent('download');await checkpoint.getByRole('button',{name:'Export JSON',exact:true}).click();const file=await (await downloadReady).path();const saved=JSON.parse(await readFile(file!,'utf8'));
 expect(saved.objective).toBe(objective);expect(saved.integrity_hash).toMatch(/^[0-9a-f]{64}$/);const ordered=Object.fromEntries(['schema_version','id','session_id','project_id','objective','requirements','constraints','decisions','changed_files','verified_results','unresolved_issues','next_steps','recovery_references','approval_boundaries','created_at','source_revision','source_usage','integrity_status','integrity_hash','continuation'].map(key=>[key,key==='integrity_hash'?'':key==='source_usage'?Object.fromEntries(['tokens','method','measurement','uncertainty'].map(field=>[field,saved.source_usage[field]])):saved[key]]));expect(createHash('sha256').update(JSON.stringify(ordered)).digest('hex')).toBe(saved.integrity_hash);expect(saved.continuation).toContain('Never upload private notes');
 await page.reload();await navigate(page,'Sessions');await page.locator('.session-item').filter({hasText:objective}).click();await expect(page.getByRole('heading',{name:objective,exact:true})).toBeVisible();await expect(page.locator('.checkpoint')).toHaveCount(1);
 page.once('dialog',dialog=>dialog.accept());await page.locator('.checkpoint').first().getByRole('button',{name:'Restore',exact:true}).click();await expect(page.getByText('Checkpoint restored into a session.',{exact:true})).toBeVisible();
 await page.reload();await navigate(page,'Sessions');await expect(page.locator('.session-item')).toContainText(objective);
 await navigate(page,'Projects');
 await page.getByLabel('Project name',{exact:true}).fill('Synthetic browser fixture');
 await page.getByLabel('Or choose text files',{exact:true}).setInputFiles([{name:'hero.css',mimeType:'text/css',buffer:Buffer.from('.hero { min-height: 20rem; color: teal; }')},{name:'.env',mimeType:'text/plain',buffer:Buffer.from('SYNTHETIC_SECRET=not-a-real-credential')}]);
 await expect(page.getByText('Saved 1 text files in this browser. 1 excluded or unsupported files skipped. No files were uploaded.',{exact:true})).toBeVisible();
 await page.getByLabel('Find context',{exact:true}).fill('hero');await page.getByRole('button',{name:'Search project files',exact:true}).click();
 await page.getByRole('checkbox',{name:'hero.css',exact:true}).check();await page.getByText('Review file text',{exact:true}).click();await expect(page.locator('pre')).toContainText('.hero { min-height: 20rem; color: teal; }');
 await page.getByRole('combobox',{name:'Add selected files to session',exact:true}).selectOption({label:objective});await page.getByRole('button',{name:'Attach selected files',exact:true}).click();await expect(page.getByText('Selected file context saved to your session. Original files were not changed.',{exact:true})).toBeVisible();
 const backupReady=page.waitForEvent('download');await page.getByRole('button',{name:'Export browser backup',exact:true}).click();const backup=JSON.parse(await readFile((await (await backupReady).path())!,'utf8'));
 expect(backup.workspace.projects).toHaveLength(1);expect(backup.workspace.projects[0].files).toHaveLength(1);expect(backup.workspace.projects[0].files[0].path).toBe('hero.css');expect(JSON.stringify(backup)).not.toContain('SYNTHETIC_SECRET');
 expect(backup.workspace.sessions.some((s:{messages:{text:string}[]})=>s.messages.some(m=>m.text.includes('File: hero.css')))).toBe(true);
 const invalid={...saved,continuation:saved.continuation+'tampered'};
 await page.getByLabel('Import checkpoint or browser backup',{exact:true}).setInputFiles({name:'invalid-checkpoint.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(invalid))});await expect(page.locator('.alert.error[role=alert]')).toBeVisible();
 const afterReady=page.waitForEvent('download');await page.getByRole('button',{name:'Export browser backup',exact:true}).click();const after=JSON.parse(await readFile((await (await afterReady).path())!,'utf8'));expect(after.workspace).toEqual(backup.workspace);
 await page.reload();await navigate(page,'Projects');await expect(page.getByRole('combobox',{name:'Project',exact:true})).toContainText('Synthetic browser fixture');
 for(const [filename,data] of [['workspace.json',backup],['checkpoint.json',saved]] as const){
  const context=await browser.newContext({baseURL});
  try{const imported=await context.newPage();const check=watchPrivacy(imported,baseURL!);await open(imported);await navigate(imported,'Projects');await imported.getByLabel('Import checkpoint or browser backup',{exact:true}).setInputFiles({name:filename,mimeType:'application/json',buffer:Buffer.from(JSON.stringify(data))});await expect(imported.getByRole('status')).toContainText(filename==='workspace.json'?'Backup imported.':'Checkpoint imported');await navigate(imported,'Sessions');await imported.locator('.session-item').filter({hasText:objective}).click();await expect(imported.getByRole('heading',{name:objective,exact:true})).toBeVisible();await expect(imported.locator('.checkpoint').first()).toContainText(objective);await imported.reload();await expect(imported.locator('.session-item')).toContainText(objective);check()}finally{await context.close()}
 }
 if(process.env.BROWSER_SCREENSHOT){
  await navigate(page,'Sessions');await page.locator('.session-item').filter({hasText:objective}).click();
  await page.screenshot({path:'../../.local-ux/browser-functional.png',fullPage:true});
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:'../../.local-ux/browser-functional-mobile.png',fullPage:true});
 }
 verifyPrivacy();
});

test('browser workspace is responsive and keeps local-only navigation usable',async({page,baseURL})=>{
 const verifyPrivacy=watchPrivacy(page,baseURL!);await open(page);
 for(const width of [320,360,390,430,768,1024,1280,1440]){
  await page.setViewportSize({width,height:900});for(const tab of ['Home','Projects','Sessions','Prompts','Usage','Connections','Settings']){await navigate(page,tab);expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),`${tab} at ${width}px`).toBe(true)}
 }
 const theme=page.getByRole('button',{name:/Switch to (light|dark) theme/});const old=await theme.getAttribute('aria-label');await theme.click();await expect(theme).not.toHaveAttribute('aria-label',old!);
 const changed=await theme.getAttribute('aria-label');await page.reload();await expect(theme).toHaveAttribute('aria-label',changed!);
 await page.goto('/#sessions');await expect(page.getByRole('button',{name:'New session',exact:true})).toBeVisible();await navigate(page,'Home');await page.goBack();await expect(page).toHaveURL(/#sessions$/);
 await page.getByRole('link',{name:'Skip to content',exact:true}).focus();await page.keyboard.press('Enter');await expect(page.locator('main')).toBeFocused();verifyPrivacy();
});

test('two browser tabs preserve independently saved sessions',async({page,context,baseURL})=>{
 const verify=watchPrivacy(page,baseURL!);await open(page);const second=await context.newPage();const verifySecond=watchPrivacy(second,baseURL!);await open(second);
 const create=async(tab:Page,title:string)=>{await navigate(tab,'Sessions');await tab.getByRole('button',{name:'New session',exact:true}).click();await tab.locator('[name=objective]').fill(title);await tab.getByRole('button',{name:'Create session',exact:true}).click();await expect(tab.getByRole('heading',{name:title,exact:true})).toBeVisible()};
 await Promise.all([create(page,'First tab synthetic task'),create(second,'Second tab synthetic task')]);await page.reload();await navigate(page,'Sessions');await expect(page.locator('.session-item')).toHaveCount(2);await expect(page.locator('.session-list')).toContainText('First tab synthetic task');await expect(page.locator('.session-list')).toContainText('Second tab synthetic task');verify();verifySecond();
});
