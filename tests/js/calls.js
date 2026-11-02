(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const $=id=>document.getElementById(id);
const ready=async()=>{await wait(50);for(let i=0;i<200&&!document.body.dataset.ready;i++)await wait(100)};
const ch=async(id,v)=>{const e=$(id);if(e.type==='checkbox')e.checked=v;else e.value=v;e.dispatchEvent(new Event('change'));await ready()};
const texts=sel=>[...document.querySelectorAll(sel)].map(x=>x.textContent);
const groups=()=>[...document.querySelectorAll('#g g[data-id]')].map(g=>{const m=/translate\(([-\d.e]+),([-\d.e]+)\)/.exec(g.getAttribute('transform'));
 return{id:g.dataset.id,x:+m[1],y:+m[2],text:[...g.querySelectorAll('text')].map(t=>t.textContent)}});
const edges=()=>[...document.querySelectorAll('#g path[data-from]')].map(p=>[p.dataset.from,p.dataset.to]);
const snap=()=>({groups:groups(),edges:edges(),badges:texts('.badge text'),stepNums:texts('#steps-list .n'),stepLines:texts('#steps-list li .lines'),
 note:$('steps-note').textContent,sub:$('steps-sub').textContent,hint:$('hint').textContent,stats:$('stats').textContent,stepsDisabled:$('k-steps').disabled,
 granDisabled:$('gran').disabled,execFid:window.__cg.state.execFid});
const opts=()=>[...$('focus').options].map(o=>[o.value,o.textContent]);
const log={};
await ready();
await ch('fdepth','1');await ch('fdir','out');await ch('focus','pkg/app::Service.run');log.s1=snap();
await ch('fdepth','2');await ch('view','exec');log.s2=snap();
await ch('focus','pkg/rec::top');await ch('fdepth','6');log.s3=snap();
await ch('view','acyclic');await ch('focus','');log.s4=snap();
await ch('view','graph');await ch('gran','class');log.s6=snap();
await ch('gran','function');await ch('view','exec');log.s5=snap();
await ch('view','graph');await ch('focus','');log.s7=snap();
await ch('focus','pkg/priv::public_fn');log.s7b=snap();
await ch('focus','');await ch('k-private',true);log.s8=snap();await ch('focus','pkg/priv::public_fn');log.s8b=snap();
await ch('focus','');await ch('mincalls','5');log.s9=Object.assign(snap(),{opts:opts()});
await ch('mincalls','3');log.s9b=Object.assign(snap(),{opts:opts()});
await ch('mincalls','0');await ch('focus','pkg/util::Tool.work');await ch('mincalls','5');log.s9c=Object.assign(snap(),{opts:opts()});
await ch('mincalls','0');await ch('focus','');await ch('view','exec');log.s10=Object.assign(snap(),{opts:opts()});
await ch('view','acyclic');log.s11=snap();
await ch('gran','class');log.s11b=snap();
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
