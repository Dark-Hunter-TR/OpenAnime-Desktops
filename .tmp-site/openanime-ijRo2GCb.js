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

import{s as K,m as D,e as V,n as A,a as C,b as H,o as N,d,q as P,r as j,f as L,D as _,i as B,v,x as y,A as M,t as F,p as G,B as J}from"./openanime-sATobStx.js";import{S as O,i as Q,c as k,d as b,m as p,t as q,a as I,f as S}from"./openanime-D6JUTAw9.js";import{T as z,g as R}from"./openanime-LfDqg_SJ.js";import{I as U}from"./openanime-CAolNVBd.js";/* empty css                  */function W(i){let s;return{c(){s=F(i[0])},l(e){s=G(e,i[0])},m(e,a){B(e,s,a)},p(e,a){a&1&&J(s,e[0])},d(e){e&&d(s)}}}function X(i){let s;return{c(){s=F(i[1])},l(e){s=G(e,i[1])},m(e,a){B(e,s,a)},p(e,a){a&2&&J(s,e[1])},d(e){e&&d(s)}}}function Y(i){let s,e,a,m,r,u,l,h,c;a=new U({props:{id:"image",src:i[5],alt:i[0]}}),r=new z({props:{variant:"subtitle",$$slots:{default:[W]},$$scope:{ctx:i}}}),l=new z({props:{class:"text-tertiary",$$slots:{default:[X]},$$scope:{ctx:i}}});let g=[{class:h="setsuki "+i[2]},i[6]],f={};for(let t=0;t<g.length;t+=1)f=D(f,g[t]);return{c(){s=V("div"),e=V("div"),k(a.$$.fragment),m=A(),k(r.$$.fragment),u=A(),k(l.$$.fragment),this.h()},l(t){s=C(t,"DIV",{class:!0,style:!0});var n=H(s);e=C(n,"DIV",{class:!0});var o=H(e);b(a.$$.fragment,o),m=N(o),b(r.$$.fragment,o),u=N(o),b(l.$$.fragment,o),o.forEach(d),n.forEach(d),this.h()},h(){P(e,f),j(e,"svelte-10oc5q5",!0),L(s,"class","contain svelte-10oc5q5"),_(s,"--s-width",i[3]),_(s,"--s-height",i[4])},m(t,n){B(t,s,n),v(s,e),p(a,e,null),v(e,m),p(r,e,null),v(e,u),p(l,e,null),c=!0},p(t,[n]){const o={};n&1&&(o.alt=t[0]),a.$set(o);const E={};n&257&&(E.$$scope={dirty:n,ctx:t}),r.$set(E);const T={};n&258&&(T.$$scope={dirty:n,ctx:t}),l.$set(T),P(e,f=R(g,[(!c||n&4&&h!==(h="setsuki "+t[2]))&&{class:h},n&64&&t[6]])),j(e,"svelte-10oc5q5",!0),(!c||n&8)&&_(s,"--s-width",t[3]),(!c||n&16)&&_(s,"--s-height",t[4])},i(t){c||(q(a.$$.fragment,t),q(r.$$.fragment,t),q(l.$$.fragment,t),c=!0)},o(t){I(a.$$.fragment,t),I(r.$$.fragment,t),I(l.$$.fragment,t),c=!1},d(t){t&&d(s),S(a),S(r),S(l)}}}function Z(i,s,e){const a=["title","description","variant","class","width","height"];let m=y(s,a),{title:r="Hayırr!"}=s,{description:u="???"}=s,{variant:l="chibi/crying"}=s;const h=`/setsuki/${l}.png`;let{class:c=""}=s,{width:g="250px"}=s,{height:f="250px"}=s;return i.$$set=t=>{s=D(D({},s),M(t)),e(6,m=y(s,a)),"title"in t&&e(0,r=t.title),"description"in t&&e(1,u=t.description),"variant"in t&&e(7,l=t.variant),"class"in t&&e(2,c=t.class),"width"in t&&e(3,g=t.width),"height"in t&&e(4,f=t.height)},[r,u,c,g,f,h,m,l]}class st extends O{constructor(s){super(),Q(this,s,Z,Y,K,{title:0,description:1,variant:7,class:2,width:3,height:4})}}export{st as S};
