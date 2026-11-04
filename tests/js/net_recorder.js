// Installed before any script of the page runs: records every attempt to reach outside the page.
(()=>{
const seen=window.__net=[];
const note=(kind,what)=>seen.push(kind+': '+String(what));
const wrap=(obj,name,kind)=>{const f=obj&&obj[name];if(typeof f!=='function')return;obj[name]=function(...a){note(kind,a[0]);return f.apply(this,a)}};
wrap(window,'fetch','fetch');wrap(XMLHttpRequest.prototype,'open','xhr');wrap(navigator,'sendBeacon','beacon');
for(const n of ['WebSocket','EventSource','Worker','SharedWorker']){const C=window[n];if(typeof C!=='function')continue;
 window[n]=function(...a){note(n,a[0]);return new C(...a)};window[n].prototype=C.prototype}
const d=Object.getOwnPropertyDescriptor(HTMLImageElement.prototype,'src');
if(d&&d.set)Object.defineProperty(HTMLImageElement.prototype,'src',{get:d.get,set(v){note('img',v);d.set.call(this,v)}});
const URLISH=['src','href','data','poster','action'];
new MutationObserver(ms=>{for(const m of ms){
 for(const n of m.addedNodes||[]){if(n.nodeType!==1)continue;for(const e of [n,...n.querySelectorAll('*')])
  for(const a of URLISH){const v=e.getAttribute&&e.getAttribute(a);if(v&&e.tagName!=='A'&&!v.startsWith('#'))note('element <'+e.tagName.toLowerCase()+' '+a+'>',v)}}
 if(m.type==='attributes'&&URLISH.includes(m.attributeName)&&m.target.tagName!=='A'){const v=m.target.getAttribute(m.attributeName);if(v&&!v.startsWith('#'))note('attribute '+m.attributeName,v)}}})
 .observe(document,{childList:true,subtree:true,attributes:true});
})();
