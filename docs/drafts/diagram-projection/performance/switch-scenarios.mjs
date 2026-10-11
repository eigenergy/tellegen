/** Explicit display scenarios; never infer switch state from an equipment name. */
export function parseSwitchCommands(text,model){
  const lookup=new Map();
  model.branches.forEach((b,i)=>{if(b.family==='switch'){
    const key=b.id.toLowerCase();if(lookup.has(key))throw new Error('Ambiguous case-insensitive switch ID');lookup.set(key,i);
  }});
  const commands=[],seen=new Set();
  for(const [lineNumber,line] of text.split(/\r?\n/).entries()){
    const code=line.split('!')[0].trim();if(!/^(open|close)\b/i.test(code))continue;
    const m=/^(open|close)\s+line\.([^\s]+)\s+term=1\s*$/i.exec(code);
    if(!m)throw new Error(`Unsupported switch command at line ${lineNumber+1}`);
    const branch=lookup.get(m[2].toLowerCase());if(branch===undefined)throw new Error(`Unknown switch ${m[2]}`);
    if(seen.has(branch))throw new Error(`Duplicate switch command ${m[2]}`);seen.add(branch);
    commands.push({id:model.branches[branch].id,open:m[1].toLowerCase()==='open',line:lineNumber+1});
  }
  if(!commands.length)throw new Error('No explicit switch commands');
  return commands;
}
export function applySwitchScenario(model,kind,commands=[],provenance={}){
  if(!['documented','all-closed'].includes(kind))throw new Error('Unknown switch scenario');
  const overrides=new Map(),switchIds=new Set(model.branches.filter(b=>b.family==='switch').map(b=>b.id));
  for(const c of commands){if(!switchIds.has(c.id)||overrides.has(c.id)||typeof c.open!=='boolean')throw new Error('Invalid switch override');overrides.set(c.id,c.open);}
  if(kind==='documented'&&!commands.length)throw new Error('Documented scenario needs explicit commands');
  let changed=0;
  const branches=model.branches.map(b=>{
    if(b.family!=='switch')return {...b};
    const openSwitch=kind==='all-closed'?false:overrides.get(b.id)??b.sourceOpenSwitch;
    if(openSwitch!==b.sourceOpenSwitch)changed++;
    return {...b,openSwitch,disabled:b.outOfService||openSwitch};
  });
  return {...model,branches,scenario:{id:kind,label:kind==='documented'?'Documented open ties':'All switches closed',
    policy:kind==='documented'?'Explicit Master.dss commands override imported state; all other switches retain imported state.':'Force every switch closed; preserve out-of-service status.',
    changedFromSource:changed,commandCount:kind==='documented'?commands.length:0,...provenance}};
}
