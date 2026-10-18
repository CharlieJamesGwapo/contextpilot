import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';
const localChrome='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const executablePath=process.env.CHROMIUM_PATH||(existsSync(localChrome)?localChrome:undefined);
export default defineConfig({
  testDir:'./tests',
  fullyParallel:false,
  workers:1,
  retries:0,
  timeout:90_000,
  expect:{timeout:10_000},
  reporter:'list',
  use:{headless:true,viewport:{width:1440,height:1100},launchOptions:{executablePath},trace:'off',screenshot:'off',video:'off'},
});
