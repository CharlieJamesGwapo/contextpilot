import {createServer} from 'node:http';
import {readFile,stat} from 'node:fs/promises';
import {resolve,sep,extname} from 'node:path';
const root=resolve('hosted-out');
const types={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.json':'application/json','.txt':'text/plain; charset=utf-8'};
const server=createServer(async(req,res)=>{
 try{
  const requested=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));
  if(requested!==root&&!requested.startsWith(root+sep)){res.writeHead(403).end();return}
  let file=requested;
  if((await stat(file)).isDirectory())file=resolve(file,'index.html');
  const bytes=await readFile(file);
  res.writeHead(200,{'Content-Type':types[extname(file)]||'application/octet-stream','Cache-Control':'no-store'}).end(bytes);
 }catch{res.writeHead(404,{'Content-Type':'text/plain'}).end('Not found')}
});
server.listen(4388,'127.0.0.1');
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>server.close(()=>process.exit(0)));
