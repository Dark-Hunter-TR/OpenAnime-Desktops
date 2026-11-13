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

import{s as I,m as b,e as k,n as M,a as E,b as T,o as O,d as g,q as v,r as y,f as q,i as L,v as N,h as U,N as V,O as z,x as S,V as D,M as G,A as H,c as J,u as K,j as Q,k as R,l as A}from"./openanime-sATobStx.js";import{S as W,i as X,t as m,g as Y,a as p,e as Z}from"./openanime-D6JUTAw9.js";import{g as w,k as x}from"./openanime-LfDqg_SJ.js";function P(i){let e,l;const f=i[10].default,u=J(f,i,i[9],null);return{c(){e=k("span"),u&&u.c(),this.h()},l(t){e=E(t,"SPAN",{class:!0});var o=T(e);u&&u.l(o),o.forEach(g),this.h()},h(){q(e,"class","svelte-wpiyrh")},m(t,o){L(t,e,o),u&&u.m(e,null),l=!0},p(t,o){u&&u.p&&(!l||o&512)&&K(u,f,t,t[9],l?R(f,t[9],o,null):Q(t[9]),null)},i(t){l||(m(u,t),l=!0)},o(t){p(u,t),l=!1},d(t){t&&g(e),u&&u.d(t)}}}function $(i){let e,l,f,u,t,o,_,r=[{class:f="toggle-switch "+i[5]},{type:"checkbox"},{__value:i[3]},{disabled:i[4]},i[7]],h={};for(let a=0;a<r.length;a+=1)h=b(h,r[a]);let s=i[8].default&&P(i);return{c(){e=k("label"),l=k("input"),u=M(),s&&s.c(),this.h()},l(a){e=E(a,"LABEL",{class:!0});var c=T(e);l=E(c,"INPUT",{class:!0,type:!0}),u=O(c),s&&s.l(c),c.forEach(g),this.h()},h(){v(l,h),y(l,"svelte-wpiyrh",!0),q(e,"class","toggle-switch-container svelte-wpiyrh")},m(a,c){L(a,e,c),N(e,l),l.autofocus&&l.focus(),l.checked=i[0],i[12](l),N(e,u),s&&s.m(e,null),i[13](e),t=!0,o||(_=[U(i[6].call(null,l)),V(l,"change",i[11])],o=!0)},p(a,[c]){v(l,h=w(r,[(!t||c&32&&f!==(f="toggle-switch "+a[5]))&&{class:f},{type:"checkbox"},(!t||c&8)&&{__value:a[3]},(!t||c&16)&&{disabled:a[4]},c&128&&a[7]])),c&1&&(l.checked=a[0]),y(l,"svelte-wpiyrh",!0),a[8].default?s?(s.p(a,c),c&256&&m(s,1)):(s=P(a),s.c(),m(s,1),s.m(e,null)):s&&(Y(),p(s,1,1,()=>{s=null}),Z())},i(a){t||(m(s),t=!0)},o(a){p(s),t=!1},d(a){a&&g(e),i[12](null),s&&s.d(),i[13](null),o=!1,z(_)}}}function ee(i,e,l){const f=["checked","value","disabled","class","inputElement","containerElement"];let u=S(e,f),{$$slots:t={},$$scope:o}=e;const _=D(t);let{checked:r=!1}=e,{value:h=void 0}=e,{disabled:s=!1}=e,{class:a=""}=e,{inputElement:c=null}=e,{containerElement:d=null}=e;const j=x(G());function B(){r=this.checked,l(0,r)}function C(n){A[n?"unshift":"push"](()=>{c=n,l(1,c)})}function F(n){A[n?"unshift":"push"](()=>{d=n,l(2,d)})}return i.$$set=n=>{e=b(b({},e),H(n)),l(7,u=S(e,f)),"checked"in n&&l(0,r=n.checked),"value"in n&&l(3,h=n.value),"disabled"in n&&l(4,s=n.disabled),"class"in n&&l(5,a=n.class),"inputElement"in n&&l(1,c=n.inputElement),"containerElement"in n&&l(2,d=n.containerElement),"$$scope"in n&&l(9,o=n.$$scope)},[r,c,d,h,s,a,j,u,_,o,t,B,C,F]}class ae extends W{constructor(e){super(),X(this,e,ee,$,I,{checked:0,value:3,disabled:4,class:5,inputElement:1,containerElement:2})}}export{ae as T};
