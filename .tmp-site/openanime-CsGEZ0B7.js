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

import{s as C,m as P,P as b,Q as E,b as y,d,f as i,R as q,r as v,i as S,v as N,E as T,x as A,A as Q,l as k}from"./openanime-sATobStx.js";import{g as R}from"./openanime-LfDqg_SJ.js";import{S as j,i as w}from"./openanime-D6JUTAw9.js";function z(a){let e;return{c(){e=b("rect"),this.h()},l(t){e=E(t,"rect",{height:!0,ry:!0,class:!0}),y(e).forEach(d),this.h()},h(){i(e,"height","3"),i(e,"ry","3"),i(e,"class","progress-bar-track svelte-19hilkx")},m(t,u){S(t,e,u),a[8](e)},p:T,d(t){t&&d(e),a[8](null)}}}function D(a){let e;return{c(){e=b("rect"),this.h()},l(t){e=E(t,"rect",{height:!0,rx:!0,y:!0,width:!0,class:!0}),y(e).forEach(d),this.h()},h(){i(e,"height","1"),i(e,"rx","0.5"),i(e,"y","1"),i(e,"width","100%"),i(e,"class","progress-bar-rail svelte-19hilkx")},m(t,u){S(t,e,u),a[7](e)},p:T,d(t){t&&d(e),a[7](null)}}}function F(a){let e,t,u,m,f,o;function g(l,n){return l[4]?D:z}let h=g(a),s=h(a),c=[{class:m="progress-bar "+a[5]},{role:"progressbar"},{width:"100%"},{height:"3"},{"aria-valuemin":f=a[4]?0:void 0},{"aria-valuemax":o=a[4]?100:void 0},{"aria-valuenow":a[4]},a[6]],_={};for(let l=0;l<c.length;l+=1)_=P(_,c[l]);return{c(){e=b("svg"),s.c(),t=b("rect"),this.h()},l(l){e=E(l,"svg",{class:!0,role:!0,width:!0,height:!0,"aria-valuemin":!0,"aria-valuemax":!0,"aria-valuenow":!0});var n=y(e);s.l(n),t=E(n,"rect",{width:!0,height:!0,rx:!0,class:!0}),y(t).forEach(d),n.forEach(d),this.h()},h(){i(t,"width",u=a[4]?`${a[4]}%`:void 0),i(t,"height","3"),i(t,"rx","1.5"),i(t,"class","progress-bar-track svelte-19hilkx"),q(e,_),v(e,"indeterminate",!a[4]),v(e,"svelte-19hilkx",!0)},m(l,n){S(l,e,n),s.m(e,null),N(e,t),a[9](t),a[10](e)},p(l,[n]){h===(h=g(l))&&s?s.p(l,n):(s.d(1),s=h(l),s&&(s.c(),s.m(e,t))),n&16&&u!==(u=l[4]?`${l[4]}%`:void 0)&&i(t,"width",u),q(e,_=R(c,[n&32&&m!==(m="progress-bar "+l[5])&&{class:m},{role:"progressbar"},{width:"100%"},{height:"3"},n&16&&f!==(f=l[4]?0:void 0)&&{"aria-valuemin":f},n&16&&o!==(o=l[4]?100:void 0)&&{"aria-valuemax":o},n&16&&{"aria-valuenow":l[4]},n&64&&l[6]])),v(e,"indeterminate",!l[4]),v(e,"svelte-19hilkx",!0)},i:T,o:T,d(l){l&&d(e),s.d(),a[9](null),a[10](null)}}}function G(a,e,t){const u=["value","class","element","railElement","trackElement","secondaryTrackElement"];let m=A(e,u),{value:f=void 0}=e,{class:o=""}=e,{element:g=null}=e,{railElement:h=null}=e,{trackElement:s=null}=e,{secondaryTrackElement:c=null}=e;function _(r){k[r?"unshift":"push"](()=>{h=r,t(1,h)})}function l(r){k[r?"unshift":"push"](()=>{c=r,t(3,c)})}function n(r){k[r?"unshift":"push"](()=>{s=r,t(2,s)})}function B(r){k[r?"unshift":"push"](()=>{g=r,t(0,g)})}return a.$$set=r=>{e=P(P({},e),Q(r)),t(6,m=A(e,u)),"value"in r&&t(4,f=r.value),"class"in r&&t(5,o=r.class),"element"in r&&t(0,g=r.element),"railElement"in r&&t(1,h=r.railElement),"trackElement"in r&&t(2,s=r.trackElement),"secondaryTrackElement"in r&&t(3,c=r.secondaryTrackElement)},[g,h,s,c,f,o,m,_,l,n,B]}class K extends j{constructor(e){super(),w(this,e,G,F,C,{value:4,class:5,element:0,railElement:1,trackElement:2,secondaryTrackElement:3})}}export{K as P};
