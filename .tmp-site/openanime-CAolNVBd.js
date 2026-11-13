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

import{s as S,F as R,i as y,E,d as n,x as q,m as b,A as L,e as v,a as j,b as V,f,r as m,q as g,N,U as M,O}from"./openanime-sATobStx.js";import{g as A}from"./openanime-LfDqg_SJ.js";import{S as P,i as U}from"./openanime-D6JUTAw9.js";/* empty css                  */function B(o){let e,l,i,a,t,u=[o[11],{src:l=o[0]},{style:i="border-radius: "+o[3]}],r={};for(let s=0;s<u.length;s+=1)r=b(r,u[s]);return{c(){e=v("img"),this.h()},l(s){e=j(s,"IMG",{src:!0,style:!0}),this.h()},h(){g(e,r),m(e,"svelte-zi2j2b",!0)},m(s,c){y(s,e,c),a||(t=[N(e,"load",o[9]),N(e,"error",o[10])],a=!0)},p(s,c){g(e,r=A(u,[c&2048&&s[11],c&1&&!M(e.src,l=s[0])&&{src:l},c&8&&i!==(i="border-radius: "+s[3])&&{style:i}])),m(e,"svelte-zi2j2b",!0)},d(s){s&&n(e),a=!1,O(t)}}}function H(o){let e,l,i=[o[11],{style:l="width: 100%; height: 100%; border-radius: "+o[3]+";"}],a={};for(let t=0;t<i.length;t+=1)a=b(a,i[t]);return{c(){e=v("div"),this.h()},l(t){e=j(t,"DIV",{style:!0}),V(e).forEach(n),this.h()},h(){g(e,a),m(e,"svelte-zi2j2b",!0)},m(t,u){y(t,e,u)},p(t,u){g(e,a=A(i,[u&2048&&t[11],u&8&&l!==(l="width: 100%; height: 100%; border-radius: "+t[3]+";")&&{style:l}])),m(e,"svelte-zi2j2b",!0)},d(t){t&&n(e)}}}function D(o){let e,l,i;function a(r,s){return r[5]?H:B}let t=a(o),u=t(o);return{c(){e=v("div"),u.c(),this.h()},l(r){e=j(r,"DIV",{class:!0,id:!0,style:!0});var s=V(e);u.l(s),s.forEach(n),this.h()},h(){f(e,"class",l="image-wrapper no-select "+o[1]+" svelte-zi2j2b"),f(e,"id",o[2]),f(e,"style",i="border-radius: "+o[3]+"; "+o[4]+"; aspect-ratio: "+o[6]+";"),m(e,"loaded",o[8])},m(r,s){y(r,e,s),u.m(e,null)},p(r,s){t===(t=a(r))&&u?u.p(r,s):(u.d(1),u=t(r),u&&(u.c(),u.m(e,null))),s&2&&l!==(l="image-wrapper no-select "+r[1]+" svelte-zi2j2b")&&f(e,"class",l),s&4&&f(e,"id",r[2]),s&88&&i!==(i="border-radius: "+r[3]+"; "+r[4]+"; aspect-ratio: "+r[6]+";")&&f(e,"style",i),s&258&&m(e,"loaded",r[8])},d(r){r&&n(e),u.d()}}}function J(o){let e=o[7],l,i=D(o);return{c(){i.c(),l=R()},l(a){i.l(a),l=R()},m(a,t){i.m(a,t),y(a,l,t)},p(a,[t]){t&128&&S(e,e=a[7])?(i.d(1),i=D(a),i.c(),i.m(l.parentNode,l)):i.p(a,t)},i:E,o:E,d(a){a&&n(l),i.d(a)}}}function K(o,e,l){const i=["class","retryCount","id","radius","src","fallback","style","loading","aspectRatio"];let a=q(e,i),{class:t}=e,{retryCount:u=2}=e,{id:r}=e,{radius:s="var(--fds-overlay-corner-radius)"}=e,{src:c=""}=e,{fallback:h="/placeholders/no-image.png"}=e,{style:z=""}=e,_=0,{loading:C=!1}=e,{aspectRatio:I="unset"}=e;c||(c=h);let k=!1;function F(){l(8,k=!0)}function G(){_<u?l(7,_++,_):h&&(l(8,k=!0),l(0,c="/placeholders/no-image.png"))}return o.$$set=d=>{e=b(b({},e),L(d)),l(11,a=q(e,i)),"class"in d&&l(1,t=d.class),"retryCount"in d&&l(12,u=d.retryCount),"id"in d&&l(2,r=d.id),"radius"in d&&l(3,s=d.radius),"src"in d&&l(0,c=d.src),"fallback"in d&&l(13,h=d.fallback),"style"in d&&l(4,z=d.style),"loading"in d&&l(5,C=d.loading),"aspectRatio"in d&&l(6,I=d.aspectRatio)},[c,t,r,s,z,C,I,_,k,F,G,a,u,h]}class Y extends P{constructor(e){super(),U(this,e,K,J,S,{class:1,retryCount:12,id:2,radius:3,src:0,fallback:13,style:4,loading:5,aspectRatio:6})}}export{Y as I};
