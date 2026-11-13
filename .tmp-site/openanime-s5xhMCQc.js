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

import{s as O,c as z,a7 as I,n as T,e as q,a8 as H,o as N,a as j,b as v,d as b,f as g,D as u,a2 as C,i as D,a3 as E,N as P,u as V,j as W,k as B}from"./openanime-sATobStx.js";import{S as G,i as J,t as K,a as L}from"./openanime-D6JUTAw9.js";function A(i){return`<style>${i}</style>`}function Q(i){let n,a=A(`
    @keyframes shimmer-mask-${i[8]} {
      0% { mask-position: 100% center; -webkit-mask-position: 100% center; }
      ${i[5]}% { mask-position: 0% center; -webkit-mask-position: 0% center; }
      100% { mask-position: 0% center; -webkit-mask-position: 0% center; }
    }`)+"",r,t,c,k,m,f,d;const _=i[16].default,l=z(_,i,i[15],null);return{c(){n=new I(!1),r=T(),t=q("div"),l&&l.c(),this.h()},l(e){n=H(e,!1),r=N(e),t=j(e,"DIV",{class:!0,style:!0});var s=v(t);l&&l.l(s),s.forEach(b),this.h()},h(){n.a=r,g(t,"class",c="text-shimmer "+i[1]+" svelte-1lhm8mi"),u(t,"--shimmer-color",i[0]),u(t,"--spread-px",i[6]+"px"),u(t,"--anim-duration",i[3]+"s"),u(t,"--anim-name",i[4]?`shimmer-mask-${i[8]}`:"none"),C(()=>i[17].call(t))},m(e,s){n.m(a,e,s),D(e,r,s),D(e,t,s),l&&l.m(t,null),k=E(t,i[17].bind(t)),m=!0,f||(d=P(t,"animationiteration",i[7]),f=!0)},p(e,[s]){(!m||s&32)&&a!==(a=A(`
    @keyframes shimmer-mask-${e[8]} {
      0% { mask-position: 100% center; -webkit-mask-position: 100% center; }
      ${e[5]}% { mask-position: 0% center; -webkit-mask-position: 0% center; }
      100% { mask-position: 0% center; -webkit-mask-position: 0% center; }
    }`)+"")&&n.p(a),l&&l.p&&(!m||s&32768)&&V(l,_,e,e[15],m?B(_,e[15],s,null):W(e[15]),null),(!m||s&2&&c!==(c="text-shimmer "+e[1]+" svelte-1lhm8mi"))&&g(t,"class",c),(!m||s&1)&&u(t,"--shimmer-color",e[0]),(!m||s&64)&&u(t,"--spread-px",e[6]+"px"),(!m||s&8)&&u(t,"--anim-duration",e[3]+"s"),(!m||s&16)&&u(t,"--anim-name",e[4]?`shimmer-mask-${e[8]}`:"none")},i(e){m||(K(l,e),m=!0)},o(e){L(l,e),m=!1},d(e){e&&(n.d(),b(r),b(t)),l&&l.d(e),k(),f=!1,d()}}}function U(i,n,a){let r,t,c,{$$slots:k={},$$scope:m}=n,{duration:f=2}=n,{spread:d=4}=n,{allowAnimationOnMount:_=!0}=n,{repeatDelay:l=500}=n,{color:e="#ffffff"}=n,{onStopFunctionReady:s=o=>{}}=n,{class:p=""}=n,h=0,y=!1,w=!0;_||(y=!0,w=!1);const S=()=>{y=!0},F=()=>{y&&a(4,w=!1)};s(S);const M=Math.random().toString(36).substr(2,9);function R(){h=this.clientWidth,a(2,h)}return i.$$set=o=>{"duration"in o&&a(9,f=o.duration),"spread"in o&&a(10,d=o.spread),"allowAnimationOnMount"in o&&a(11,_=o.allowAnimationOnMount),"repeatDelay"in o&&a(12,l=o.repeatDelay),"color"in o&&a(0,e=o.color),"onStopFunctionReady"in o&&a(13,s=o.onStopFunctionReady),"class"in o&&a(1,p=o.class),"$$scope"in o&&a(15,m=o.$$scope)},i.$$.update=()=>{i.$$.dirty&1028&&a(6,r=h>0?h/d:0),i.$$.dirty&4608&&a(3,t=f+l/1e3),i.$$.dirty&520&&a(5,c=f/t*100)},[e,p,h,t,w,c,r,F,M,f,d,_,l,s,S,m,k,R]}class Z extends G{constructor(n){super(),J(this,n,U,Q,O,{duration:9,spread:10,allowAnimationOnMount:11,repeatDelay:12,color:0,onStopFunctionReady:13,class:1,stop:14})}get stop(){return this.$$.ctx[14]}}export{Z as T};
