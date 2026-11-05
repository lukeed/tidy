import {readFile} from 'node:fs/promises'

type Route={
path:string,run:(req:Request)=>Promise<Response>}

const routes:Route[]=[
{path:'/',run:()=>page("index.html")},
  {path:"/up",run:async()=>new Response('ok')},
]

export function match(path:string){
return routes.find(r=>r.path===path)??null}

async function page(file:string){
const body=await readFile(file,"utf8")
  const type={"content-type":'text/html'}
return new Response(body,{headers:type})}
