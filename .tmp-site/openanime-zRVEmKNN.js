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

import{s as F,c as H,m as I,e as V,n as J,a as q,b as j,o as L,d as p,q as U,r as g,i as M,v as Q,N as E,u as R,j as T,k as W,O as X,x as A,K as Y,A as Z,l as w,f as x,w as $}from"./openanime-sATobStx.js";import{S as ee,i as te,t as h,g as se,a as m,e as le,b as ae,c as ne,d as ie,m as ce,f as oe}from"./openanime-D6JUTAw9.js";import{g as re,v as C,w as fe}from"./openanime-LfDqg_SJ.js";import{C as de}from"./openanime-v9xf9uZK.js";const ue=i=>({}),D=i=>({setSelected:i[5]});function K(i){let e,l,f,u;function _(n){i[10](n)}let s={tabindex:"-1"};return i[0]!==void 0&&(s.checked=i[0]),l=new de({props:s}),w.push(()=>ae(l,"checked",_)),l.$on("change",i[11]),{c(){e=V("div"),ne(l.$$.fragment),this.h()},l(n){e=q(n,"DIV",{class:!0});var a=j(e);ie(l.$$.fragment,a),a.forEach(p),this.h()},h(){x(e,"class","item-checkbox svelte-1aippcc")},m(n,a){M(n,e,a),ce(l,e,null),u=!0},p(n,a){const o={};!f&&a&1&&(f=!0,o.checked=n[0],$(()=>f=!1)),l.$set(o)},i(n){u||(h(l.$$.fragment,n),u=!0)},o(n){m(l.$$.fragment,n),u=!1},d(n){n&&p(e),oe(l)}}}function _e(i){let e,l,f,u,_,s=!i[2]&&K(i);const n=i[9].default,a=H(n,i,i[8],D);let o=[{class:"grid-view-item"},i[7],{tabindex:"0"}],d={};for(let t=0;t<o.length;t+=1)d=I(d,o[t]);return{c(){e=V("div"),s&&s.c(),l=J(),a&&a.c(),this.h()},l(t){e=q(t,"DIV",{class:!0,tabindex:!0});var r=j(e);s&&s.l(r),l=L(r),a&&a.l(r),r.forEach(p),this.h()},h(){U(e,d),g(e,"selected",i[0]),g(e,"svelte-1aippcc",!0)},m(t,r){M(t,e,r),s&&s.m(e,null),Q(e,l),a&&a.m(e,null),f=!0,u||(_=[E(e,"click",i[6]),E(e,"keydown",i[12])],u=!0)},p(t,[r]){t[2]?s&&(se(),m(s,1,1,()=>{s=null}),le()):s?(s.p(t,r),r&4&&h(s,1)):(s=K(t),s.c(),h(s,1),s.m(e,l)),a&&a.p&&(!f||r&256)&&R(a,n,t,t[8],f?W(n,t[8],r,ue):T(t[8]),D),U(e,d=re(o,[{class:"grid-view-item"},r&128&&t[7],{tabindex:"0"}])),g(e,"selected",t[0]),g(e,"svelte-1aippcc",!0)},i(t){f||(h(s),h(a,t),f=!0)},o(t){m(s),m(a,t),f=!1},d(t){t&&p(e),s&&s.d(),a&&a.d(t),u=!1,X(_)}}}function he(i,e,l){const f=["selected","group","singleSelect","disableUnselectionIfGrouped"];let u=A(e,f),{$$slots:_={},$$scope:s}=e;var n;const a=Y();let{selected:o=!1}=e,{group:d=""}=e,{singleSelect:t=!1}=e,{disableUnselectionIfGrouped:r=!1}=e;const G=Math.random();function b(c,z){var k;if(t&&!z){const y=(k=C(`group:${d}`))!==null&&k!==void 0?k:[];if(y.length){const B=y.filter(v=>v.id!==G);for(const v of B)v.setSelected(!1,!0)}}l(0,o=c)}function S(){r&&d.length>0&&o||(b(!o,!1),a("change",{selected:o}))}if(d.length){const c=(n=C(`group:${d}`))!==null&&n!==void 0?n:[];c.push({id:G,setSelected:b}),fe(`group:${d}`,c)}function N(c){o=c,l(0,o)}const O=()=>{if(r&&d.length>0&&!o){l(0,o=!0);return}a("change",{selected:!o})},P=c=>{c.key==="Enter"&&S()};return i.$$set=c=>{e=I(I({},e),Z(c)),l(7,u=A(e,f)),"selected"in c&&l(0,o=c.selected),"group"in c&&l(1,d=c.group),"singleSelect"in c&&l(2,t=c.singleSelect),"disableUnselectionIfGrouped"in c&&l(3,r=c.disableUnselectionIfGrouped),"$$scope"in c&&l(8,s=c.$$scope)},[o,d,t,r,a,b,S,u,s,_,N,O,P]}class ke extends ee{constructor(e){super(),te(this,e,he,_e,F,{selected:0,group:1,singleSelect:2,disableUnselectionIfGrouped:3})}}export{ke as G};
