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

import{s as E,y as v,z as x,e as w,a as S,b as j,d as f,f as B,i as $,v as C,n as O,o as P,t as k,p as y,B as z}from"./openanime-sATobStx.js";import{S as F,i as T,c as d,d as g,m as b,t as p,a as _,f as h,g as A,e as I}from"./openanime-D6JUTAw9.js";import{g as D}from"./openanime-BbDyok9i.js";import{p as L}from"./openanime-D4S_47Tf.js";import{E as M}from"./openanime-DOYmocve.js";import{A as R,J as U,e as V,T as W,B as q}from"./openanime-LfDqg_SJ.js";function G(o){let t,r;return t=new W({props:{variant:"bodyStrong",$$slots:{default:[H]},$$scope:{ctx:o}}}),{c(){d(t.$$.fragment)},l(e){g(t.$$.fragment,e)},m(e,n){b(t,e,n),r=!0},p(e,n){const a={};n&8&&(a.$$scope={dirty:n,ctx:e}),t.$set(a)},i(e){r||(p(t.$$.fragment,e),r=!0)},o(e){_(t.$$.fragment,e),r=!1},d(e){h(t,e)}}}function J(o){let t,r,e,n;return t=new M({props:{$$slots:{default:[K]},$$scope:{ctx:o}}}),e=new q({props:{variant:"hyperlink",href:"/",$$slots:{default:[N]},$$scope:{ctx:o}}}),{c(){d(t.$$.fragment),r=O(),d(e.$$.fragment)},l(a){g(t.$$.fragment,a),r=P(a),g(e.$$.fragment,a)},m(a,s){b(t,a,s),$(a,r,s),b(e,a,s),n=!0},p(a,s){const l={};s&9&&(l.$$scope={dirty:s,ctx:a}),t.$set(l);const i={};s&8&&(i.$$scope={dirty:s,ctx:a}),e.$set(i)},i(a){n||(p(t.$$.fragment,a),p(e.$$.fragment,a),n=!0)},o(a){_(t.$$.fragment,a),_(e.$$.fragment,a),n=!1},d(a){a&&f(r),h(t,a),h(e,a)}}}function H(o){let t;return{c(){t=k("Odaya yönlendiriliyorsun...")},l(r){t=y(r,"Odaya yönlendiriliyorsun...")},m(r,e){$(r,t,e)},d(r){r&&f(t)}}}function K(o){let t;return{c(){t=k(o[0])},l(r){t=y(r,o[0])},m(r,e){$(r,t,e)},p(r,e){e&1&&z(t,r[0])},d(r){r&&f(t)}}}function N(o){let t;return{c(){t=k("Ana sayfaya dön")},l(r){t=y(r,"Ana sayfaya dön")},m(r,e){$(r,t,e)},d(r){r&&f(t)}}}function Q(o){let t,r,e,n,a;const s=[J,G],l=[];function i(c,u){return c[0]?0:1}return e=i(o),n=l[e]=s[e](o),{c(){t=w("div"),r=w("div"),n.c(),this.h()},l(c){t=S(c,"DIV",{class:!0});var u=j(t);r=S(u,"DIV",{class:!0});var m=j(r);n.l(m),m.forEach(f),u.forEach(f),this.h()},h(){B(r,"class","inner svelte-1kjpf53"),B(t,"class","room-page svelte-1kjpf53")},m(c,u){$(c,t,u),C(t,r),l[e].m(r,null),a=!0},p(c,u){let m=e;e=i(c),e===m?l[e].p(c,u):(A(),_(l[m],1,1,()=>{l[m]=null}),I(),n=l[e],n?n.p(c,u):(n=l[e]=s[e](c),n.c()),p(n,1),n.m(r,null))},i(c){a||(p(n),a=!0)},o(c){_(n),a=!1},d(c){c&&f(t),l[e].d()}}}function X(o){let t,r;return t=new R({props:{showFooter:!1,$$slots:{default:[Q]},$$scope:{ctx:o}}}),{c(){d(t.$$.fragment)},l(e){g(t.$$.fragment,e)},m(e,n){b(t,e,n),r=!0},p(e,[n]){const a={};n&9&&(a.$$scope={dirty:n,ctx:e}),t.$set(a)},i(e){r||(p(t.$$.fragment,e),r=!0)},o(e){_(t.$$.fragment,e),r=!1},d(e){h(t,e)}}}function Y(o,t,r){let e,n;v(o,L,s=>r(1,e=s)),v(o,U,s=>r(2,n=s));let a="";return x(async()=>{if(!(n!=null&&n.id)){r(0,a="Beraber izleme odasına katılmak için giriş yapmalısınız.");return}const s=e.params.roomCode.replace("room:","");try{const l=await fetch(`${V.PUBLIC_W2G_URL}/anime-info/${s}`);if(!l.ok)throw new window.Error("Oda bulunamadı veya artık aktif değil.");const i=await l.json();await D(`/anime/${i.slug}/${i.season}/${i.episode}`,{replaceState:!0,state:{joinRoomCode:s,preSelectedFansub:i.fansub,restoreWatchTogetherPanel:!0}})}catch(l){r(0,a=l.message)}}),[a]}class Z extends F{constructor(t){super(),T(this,t,Y,X,E,{})}}const se=Object.freeze(Object.defineProperty({__proto__:null,default:Z},Symbol.toStringTag,{value:"Module"}));export{Z as P,se as _};
