(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const state=()=>{const r=document.documentElement,cs=getComputedStyle(r),h=document.querySelector('header'),hint=document.getElementById('hint');
 return{embed:r.classList.contains('embed'),theme:r.dataset.theme||null,bg:cs.getPropertyValue('--bg').trim(),
  headerHeight:h.offsetHeight,svgHeight:Math.round(document.getElementById('v').getBoundingClientRect().height),
  hintVisible:hint?hint.offsetHeight>0:null,boxes:document.querySelectorAll('#v g').length}};
const set=async(hash)=>{location.hash=hash;await wait(300)};
const log={};
await wait(2000);log.initial=state();
await set('embed=1');log.embed=state();
await set('embed=1&theme=dark');log.embedDark=state();
await set('theme=light');log.light=state();
await set('theme=dark');log.dark=state();
await set('theme=nonsense');log.invalid=state();
await set('');log.reset=state();
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
