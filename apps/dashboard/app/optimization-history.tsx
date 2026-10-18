'use client';
import {useState} from 'react';
type Entry={id:string;created_at:string;original_tokens:number;optimized_tokens:number;tokens_reduced:number};
export function OptimizationHistory({entries}:{entries:Entry[]}) {
 const [count,setCount]=useState(10);
 return <section className="panel activity"><h2>Prompt optimization history</h2><p>Saved local text estimates. Changes to prompt length do not establish a lower provider bill.</p>{entries.length?<><div className="table-wrap"><table><caption className="sr-only">Recorded prompt optimization estimates</caption><thead><tr><th>When</th><th>Original tokens</th><th>Suggested tokens</th><th>Estimated reduction</th></tr></thead><tbody>{entries.slice().reverse().slice(0,count).map(x=><tr key={x.id}><td>{new Date(x.created_at).toLocaleString()}</td><td>{x.original_tokens.toLocaleString()}</td><td>{x.optimized_tokens.toLocaleString()}</td><td>{x.tokens_reduced.toLocaleString()}</td></tr>)}</tbody></table></div>{entries.length>count&&<button className="secondary" onClick={()=>setCount(count+10)}>Show more optimizations ({entries.length-count})</button>}</>:<p>No prompt optimizations saved yet. Try the local optimizer in Prompts.</p>}</section>
}
