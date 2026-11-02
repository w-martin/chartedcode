(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const $=id=>document.getElementById(id);
const ready=async()=>{await wait(50);for(let i=0;i<200&&!document.body.dataset.ready;i++)await wait(100)};
const ch=async(id,v)=>{const e=$(id);if(e.type==='checkbox')e.checked=v;else e.value=v;e.dispatchEvent(new Event('change'));await ready()};
const texts=sel=>[...document.querySelectorAll(sel)].map(x=>x.textContent);
const snap=()=>({
 blocks:document.querySelectorAll('#g g[data-id]').length,
 labels:texts('#g g.edgelabel text'),
 edges:document.querySelectorAll('#g path[data-from]').length,
 circles:document.querySelectorAll('#g circle').length,
 stats:$('stats').textContent,hint:$('hint').textContent,fn:$('fn').value,
 options:[...$('fn').options].map(o=>[o.value,o.textContent]),
 blockText:[...document.querySelectorAll('#g g[data-id]')].map(g=>[...g.querySelectorAll('text')].map(t=>t.textContent).join('\n'))});
const log={};
await ready();log.initial=snap();
await ch('fn','flow::branch');log.branch=snap();
await ch('fn','flow::loop_while_break');log.loop=snap();
await ch('fn','flow::infinite_loop');log.infinite=snap();
await ch('fn','flow::after_return');log.afterReturn=snap();
await ch('fn','flow::elif_chain');await ch('minc','3');log.minc3=snap();
await ch('minc','0');log.minc0=snap();
await ch('fn','flow::branch');
{const group=[...document.querySelectorAll('#g g[data-id]')].find(g=>g.textContent.includes('if x'));
 group.dispatchEvent(new PointerEvent('pointerenter',{bubbles:false}));
 const during=document.querySelectorAll('#g path.dim').length;
 group.dispatchEvent(new PointerEvent('pointerleave',{bubbles:false}));
 log.hover={during,after:document.querySelectorAll('#g path.dim').length,paths:document.querySelectorAll('#g path[data-from]').length}}
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
