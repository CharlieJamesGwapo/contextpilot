import {test,expect} from '@playwright/test';
const tabs=['Home','Projects','Sessions','Prompts','Usage','Connections','Settings'];
test('hosted browser workspace is private, responsive and independent of a local server',async({page,baseURL})=>{
 const errors:string[]=[],unexpected:string[]=[];
 page.on('pageerror',error=>errors.push(error.message));
 page.on('request',request=>{const url=new URL(request.url());if(url.origin!==new URL(baseURL!).origin||url.pathname.startsWith('/api/')||request.headers().authorization)unexpected.push(`${url.origin}${url.pathname}`)});
 await page.addInitScript(()=>sessionStorage.setItem('contextpilot-token','synthetic-hosted-isolation-test-not-a-real-token'));
 await page.goto('/');
 await expect(page.getByText('Stored in this browser',{exact:true})).toBeVisible();
 await expect(page.getByLabel('Connection code',{exact:true})).toHaveCount(0);
 for(const asset of ['/contextpilot-mark.svg','/icon.svg']){const response=await page.request.get(asset);expect(response.ok()).toBe(true);expect(await response.text()).toContain('<svg')}
 const favicon=await page.locator('link[rel=icon]').first().getAttribute('href');expect(favicon).toBeTruthy();expect((await page.request.get(favicon!)).ok()).toBe(true);
 const skip=page.getByRole('button',{name:'Skip tour',exact:true});if(await skip.isVisible())await skip.click();
 if(process.env.BROWSER_SCREENSHOT)await page.screenshot({path:'../../docs/hosted-preview.jpg',type:'jpeg',quality:85,fullPage:true});
 const theme=page.getByRole('button',{name:/Switch to (light|dark) theme/});const label=await theme.getAttribute('aria-label');await theme.click();await expect(theme).not.toHaveAttribute('aria-label',label!);const changed=await theme.getAttribute('aria-label');await page.reload();await expect(theme).toHaveAttribute('aria-label',changed!);
 for(const width of [320,360,390,430,768,1024,1280,1440]){await page.setViewportSize({width,height:900});for(const tab of tabs){const button=page.getByRole('navigation',{name:'Main navigation'}).getByRole('button',{name:tab,exact:false});await button.click();await expect(button).toHaveAttribute('aria-current','page');await expect(page).toHaveURL(new RegExp(`#${tab.toLowerCase()}$`));expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),`${tab} fits ${width}px`).toBe(true)}}
 await page.goto('/#sessions');await expect(page.getByRole('button',{name:'New session',exact:true})).toBeVisible();const home=page.getByRole('navigation',{name:'Main navigation'}).getByRole('button',{name:'Home',exact:true});await home.focus();await page.keyboard.press('Enter');await expect(page).toHaveURL(/#home$/);await page.goBack();await expect(page).toHaveURL(/#sessions$/);
 await page.getByRole('link',{name:'Skip to content',exact:true}).focus();await page.keyboard.press('Enter');await expect(page.locator('main')).toBeFocused();expect(errors).toEqual([]);expect(unexpected).toEqual([]);
});
