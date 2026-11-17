import { chromium } from 'playwright';
import { readFileSync } from 'node:fs';
// Start Vite at the repository root; pass URL, solver.wasm and a case in PowerIO JSON.
const [url, wasm, input] = process.argv.slice(2);
const browser = await chromium.launch({headless:true});
const page = await browser.newPage();
const errors=[];
page.on('pageerror',e=>errors.push(e.message));
await page.route('**/test.html',r=>r.fulfill({contentType:'text/html',body:'<!doctype html><title>AC OPF check</title>'}));
await page.route('**/solver.wasm',r=>r.fulfill({contentType:'application/wasm',body:readFileSync(wasm)}));
await page.goto(`${url}/test.html`);
const moduleJson=readFileSync(input,'utf8');
const result=await page.evaluate(async moduleJson=>{
 const {solveAcOpfModule,releaseAcOpfWorker}=await import('/packages/engine/src/acopf.ts');
 const cells=[{operand:{Dispatch:'Active'},parameter:{Demand:'Active'}}];
 const first=await solveAcOpfModule('/solver.wasm',moduleJson,undefined,undefined,cells);
 const again=await solveAcOpfModule('/solver.wasm',moduleJson,undefined,undefined,cells);
 const released=releaseAcOpfWorker();
 return {first,again,released};
},moduleJson);
if(errors.length || !result.released || result.first.sensitivities?.[0].values.length!==3 || JSON.stringify(result.first.sensitivities)!==JSON.stringify(result.again.sensitivities)) throw new Error(JSON.stringify({errors,result}));
console.log(JSON.stringify({browser:await browser.version(),status:result.first.status,objective:result.first.objective,sensitivities:result.first.sensitivities,repeated:true,released:result.released,errors}));
await browser.close();
