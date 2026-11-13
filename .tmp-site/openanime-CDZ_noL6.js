/*
* OpenAnime
* https://openani.me
* (c) 2026 OpenAnime Labs.
* Licensed under the BSL-1.1 License
* More information at https://github.com/OpenAnime/licenses/blob/main/BSL.txt
*/

/*!
 ******************* BEGIN OpenFrameGeneration 3.5 Copyright Notice ****************
 *
 * COPYRIGHT
 *
 * Copyright (c) 2025-2026, RAISE-SR Visual Computing Group, OpenAnime Labs.
 * All rights reserved.
 * 
 *
 ***************** END OpenFrameGeneration 3.5 Copyright Notice ********************
 */

import{s as R,m as b,P as E,Q as k,b as z,d as _,f as v,R as P,r as d,i as S,v as q,h as A,E as w,x as y,M as C,K as D,A as F,l as N}from"./openanime-sATobStx.js";import{g as I,k as K}from"./openanime-LfDqg_SJ.js";import{S as Q,i as V}from"./openanime-D6JUTAw9.js";function j(a){let e,s,u,o,c,i,f,g,m,n=[{tabindex:"-1"},{class:o="progress-ring "+a[4]},{width:a[3]},{height:a[3]},{viewBox:"0 0 16 16"},{role:c=a[0]?"progressbar":"status"},{"aria-valuemin":i=a[6]?void 0:0},{"aria-valuemax":f=a[6]?void 0:100},{"aria-valuenow":a[0]},a[8]],h={};for(let t=0;t<n.length;t+=1)h=b(h,n[t]);return{c(){e=E("svg"),s=E("circle"),this.h()},l(t){e=k(t,"svg",{tabindex:!0,class:!0,width:!0,height:!0,viewBox:!0,role:!0,"aria-valuemin":!0,"aria-valuemax":!0,"aria-valuenow":!0});var r=z(e);s=k(r,"circle",{cx:!0,cy:!0,r:!0,"stroke-dasharray":!0,"stroke-dashoffset":!0,class:!0}),z(s).forEach(_),r.forEach(_),this.h()},h(){v(s,"cx","50%"),v(s,"cy","50%"),v(s,"r","7"),v(s,"stroke-dasharray","3"),v(s,"stroke-dashoffset",u=(100-a[0])/100*a[5]),v(s,"class","svelte-1xv7sw"),P(e,h),d(e,"indeterminate",a[6]),d(e,"svelte-1xv7sw",!0)},m(t,r){S(t,e,r),q(e,s),a[9](s),a[10](e),g||(m=A(a[7].call(null,e)),g=!0)},p(t,[r]){r&33&&u!==(u=(100-t[0])/100*t[5])&&v(s,"stroke-dashoffset",u),P(e,h=I(n,[{tabindex:"-1"},r&16&&o!==(o="progress-ring "+t[4])&&{class:o},r&8&&{width:t[3]},r&8&&{height:t[3]},{viewBox:"0 0 16 16"},r&1&&c!==(c=t[0]?"progressbar":"status")&&{role:c},r&64&&i!==(i=t[6]?void 0:0)&&{"aria-valuemin":i},r&64&&f!==(f=t[6]?void 0:100)&&{"aria-valuemax":f},r&1&&{"aria-valuenow":t[0]},r&256&&t[8]])),d(e,"indeterminate",t[6]),d(e,"svelte-1xv7sw",!0)},i:w,o:w,d(t){t&&_(e),a[9](null),a[10](null),g=!1,m()}}}function G(a,e,s){let u;const o=["value","size","class","element","circleElement"];let c=y(e,o),{value:i=void 0}=e,{size:f=32}=e,{class:g=""}=e,{element:m=null}=e,{circleElement:n=null}=e;const h=K(C(),["change"]),t=D();let r;function B(l){N[l?"unshift":"push"](()=>{n=l,s(1,n)})}function M(l){N[l?"unshift":"push"](()=>{m=l,s(2,m)})}return a.$$set=l=>{e=b(b({},e),F(l)),s(8,c=y(e,o)),"value"in l&&s(0,i=l.value),"size"in l&&s(3,f=l.size),"class"in l&&s(4,g=l.class),"element"in l&&s(2,m=l.element),"circleElement"in l&&s(1,n=l.circleElement)},a.$$.update=()=>{a.$$.dirty&1&&(i<0?s(0,i=0):i>100&&s(0,i=100)),a.$$.dirty&1&&s(6,u=typeof i>"u"||i===null||Number.isNaN(i)),a.$$.dirty&1&&t("change",i),a.$$.dirty&2&&n&&s(5,r=Math.PI*(n.r.baseVal.value*2))},[i,n,m,f,g,r,u,h,c,B,M]}class O extends Q{constructor(e){super(),V(this,e,G,j,R,{value:0,size:3,class:4,element:2,circleElement:1})}}export{O as P};
