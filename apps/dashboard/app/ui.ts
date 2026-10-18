/** Display-only privacy filter. Never use this to rewrite saved records or exports. */
export function displayText(value:string):string {
 return value
  .replace(/(?:\/Users\/|\/home\/|[A-Z]:\\Users\\)[^\s"'<>\]\n]+/g,'[local path]')
  .replace(/(^|[\s"'(])\/(?:var|Volumes|private|tmp|opt|etc|root|mnt|media)\/[^\s"'<>\]\n]+/g,'$1[local path]')
  .replace(/\b(?:sk-[A-Za-z0-9_-]{12,}|Bearer\s+[A-Za-z0-9._-]{12,})\b/g,'[hidden credential]')
  .replace(/\b([A-Z][A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD))\s*[=:]\s*[^\s,;]+/g,'$1=[hidden credential]');
}
export function safeError(status:number):string {
 if(status===401)return 'Your connection code is missing or no longer valid. Copy the current code from the ContextPilot terminal and try again.';
 if(status===403)return 'This local connection was refused. Open ContextPilot at its announced localhost address and reconnect.';
 if(status===409)return 'The saved state changed or the context limit was reached. Refresh the session and preview again.';
 if(status===413)return 'This content is too large. Select fewer files or split the text, then try again.';
 if(status>=500)return 'ContextPilot could not finish this operation. Check that the local app is running, then retry.';
 return 'The request could not be saved. Check the required fields, folder access and configured limits, then try again. Refresh before retrying a checkpoint.';
}
export function downloadText(name:string,text:string,type='text/plain') {
 const url=URL.createObjectURL(new Blob([text],{type}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}
