(async()=>{
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const $=id=>document.getElementById(id);
const ready=async()=>{await wait(50);for(let i=0;i<200&&!document.body.dataset.ready;i++)await wait(100)};
const grp=name=>[...document.querySelectorAll('#g g[data-id]')].find(g=>g.dataset.id.endsWith('::'+name));
const ev=(t,type,x,y)=>t.dispatchEvent(new PointerEvent(type,{bubbles:true,clientX:x,clientY:y,pointerId:1}));
const click=async(t)=>{const r=t.getBoundingClientRect();const x=r.x+r.width/2,y=r.y+r.height/2;ev(t,'pointerdown',x,y);ev(t,'pointerup',x,y);await wait(300);await ready()};
const clickEmpty=async()=>{const r=$('v').getBoundingClientRect();ev($('v'),'pointerdown',r.x+4,r.y+4);ev($('v'),'pointerup',r.x+4,r.y+4);await wait(300);await ready()};
const snap=(label)=>log.push({label,hint:$('hint').textContent,hintVisible:$('hint').offsetHeight>0,focus:$('focus').value,clearDisabled:$('clear').disabled,
 armed:[...document.querySelectorAll('#g g.armed')].map(g=>g.dataset.id),focused:[...document.querySelectorAll('#g g.focused')].map(g=>g.dataset.id)});
const log=[];
await ready();snap('idle');
await click(grp('__FOCUSED__'));snap('click A once');
await click(grp('__FOCUSED__'));snap('click A twice');
await click(grp('__OTHER__'));snap('click B once while A focused');
await clickEmpty();snap('click empty once');
document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape'}));snap('Escape');
await clickEmpty();await clickEmpty();snap('click empty twice');
await click(grp('__FOCUSED__'));await click(grp('__FOCUSED__'));snap('refocus A');
await click(grp('__FOCUSED__'));snap('click focused A once');
await click(grp('__FOCUSED__'));snap('click focused A twice');
{const t=grp('__FOCUSED__').querySelector('rect'),r=t.getBoundingClientRect();const x=r.x+5,y=r.y+5;
 ev(t,'pointerdown',x,y);ev($('v'),'pointermove',x+40,y+30);ev($('v'),'pointerup',x+40,y+30);await wait(200);snap('drag from a box')}
const pre=document.createElement('pre');pre.id='log';pre.textContent=JSON.stringify(log);document.body.appendChild(pre);
})();
