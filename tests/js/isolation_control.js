(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
new Image().src='http://example.invalid/pixel.png';
fetch('http://example.invalid/data').catch(()=>{});
const s=document.createElement('script');s.src='http://example.invalid/x.js';document.head.appendChild(s);
await wait(300);
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify({attempts:window.__net});document.body.appendChild(pre);
})();
