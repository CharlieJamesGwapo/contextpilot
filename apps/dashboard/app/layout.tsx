import type { Metadata } from 'next';
import './globals.css';
import './refresh.css';
import './product.css';
export const metadata: Metadata = {title:'ContextPilot — Your project, in context',description:'Clearer prompts, useful project context, and saved progress. Private on your computer.',icons:{icon:'/contextpilot-mark.svg'}};
export default function RootLayout({children}:{children:React.ReactNode}) {return <html lang="en"><body>{children}</body></html>}
