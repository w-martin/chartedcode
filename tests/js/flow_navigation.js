(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const $=id=>document.getElementById(id);
const ready=async()=>{await wait(50);for(let i=0;i<200&&!document.body.dataset.ready;i++)await wait(100)};
const ch=async(id,v)=>{const e=$(id);if(e.type==='checkbox')e.checked=v;else e.value=v;e.dispatchEvent(new Event('change'));await ready()};
const ev=(t,type,x,y)=>t.dispatchEvent(new PointerEvent(type,{bubbles:true,clientX:x,clientY:y,pointerId:1}));
const CALLEE='pkg/app::Service.__init__';
const line=()=>document.querySelector('#g [data-callee="'+CALLEE+'"]');
const centre=t=>{const r=t.getBoundingClientRect();return[r.x+r.width/2,r.y+r.height/2]};
const click=async(t)=>{const [x,y]=centre(t);ev(t,'pointerdown',x,y);ev(t,'pointerup',x,y);await wait(300);await ready()};
const state=()=>({fn:$('fn').value,hint:$('hint').textContent,armed:window.__flow.state.armed,history:window.__flow.state.history.length,
 backDisabled:$('back').disabled,armedLines:document.querySelectorAll('#g .armedcall').length,hintVisible:$('hint').offsetHeight>0,options:$('fn').options.length});
const log={};
await ready();log.start=Object.assign(state(),{hasCalleeLine:!!line()});
await click(line());log.armed=state();
await click(line());log.opened=state();
$('back').click();await ready();log.back=state();
await click(line());document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape'}));log.escape=state();
{const t=line(),[x,y]=centre(t);ev(t,'pointerdown',x,y);ev($('v'),'pointermove',x+40,y+30);ev($('v'),'pointerup',x+40,y+30);await wait(200);log.drag=state()}
await ch('k-private',true);log.privateShown=state();
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
