(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const sizes=()=>{const out={};for(const k of ['localStorage','sessionStorage']){try{out[k]=window[k].length}catch(_){out[k]='unavailable'}}return out};
const log={};
await wait(2000);
// use the page a little: change its state and let it lay out again
for(const id of ['k-private','k-ext','k-composes','k-inherits']){const e=document.getElementById(id);if(e&&e.type==='checkbox'){e.checked=!e.checked;e.dispatchEvent(new Event('change'));await wait(400)}}
location.hash='embed=1&theme=dark';await wait(400);
log.attempts=window.__net;
log.cookie=document.cookie;
log.storage=sizes();
log.boxes=document.querySelectorAll('#v g').length;
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
