(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const $=id=>document.getElementById(id);
const ready=async()=>{await wait(50);for(let i=0;i<200&&!document.body.dataset.ready;i++)await wait(100)};
const ch=async(id,v)=>{const e=$(id);e.checked=v;e.dispatchEvent(new Event('change'));await ready()};
const snap=()=>({stats:$('stats').textContent,boxes:document.querySelectorAll('#g > g').length,
 inherits:$('k-inherits').checked,implementsOn:$('k-implements').checked,composes:$('k-composes').checked});
const log={};
await ready();log.initial=snap();
await ch('k-composes',true);log.withComposes=snap();
await ch('k-inherits',false);await ch('k-implements',false);await ch('k-composes',false);log.none=snap();
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
