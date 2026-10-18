import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const cli=fileURLToPath(new URL('../node_modules/next/dist/bin/next',import.meta.url));
const args=process.argv.slice(2);const hosted=args.includes('--hosted');
const result=spawnSync(process.execPath,[cli,...args.filter(arg=>arg!=='--hosted')],{stdio:'inherit',env:{...process.env,...(hosted?{NEXT_PUBLIC_CONTEXTPILOT_MODE:'hosted'}:{}),NEXT_TELEMETRY_DISABLED:'1'}});
if(result.error)throw result.error;
process.exit(result.status??1);
