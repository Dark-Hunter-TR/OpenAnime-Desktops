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

import{s as j,F as P,i as w,E as C,d as R,y as z,z as E,e as k,t as D,a as O,b as F,p as q,a6 as J,v as U}from"./openanime-sATobStx.js";import{S as B,i as G}from"./openanime-D6JUTAw9.js";import{F as H}from"./openanime-LfDqg_SJ.js";function A(m){let i,t=JSON.stringify({...m[1],...m[2]})+"",n;return{c(){i=k("premid-announcer"),n=D(t),this.h()},l(a){i=O(a,"PREMID-ANNOUNCER",{class:!0});var l=F(i);n=q(l,t),l.forEach(R),this.h()},h(){J(i,"class","svelte-1m2l2bl")},m(a,l){w(a,i,l),U(i,n)},p:C,d(a){a&&R(i)}}}function L(m){let i,t=!m[0]&&A(m);return{c(){t&&t.c(),i=P()},l(n){t&&t.l(n),i=P()},m(n,a){t&&t.m(n,a),w(n,i,a)},p(n,[a]){n[0]?t&&(t.d(1),t=null):t?t.p(n,a):(t=A(n),t.c(),t.m(i.parentNode,i))},i:C,o:C,d(n){n&&R(i),t&&t.d(n)}}}function Q(m,i,t){let n;z(m,H,e=>t(0,n=e));let{details:a=""}=i,{state:l=""}=i,{startTimestamp:c=void 0}=i,{endTimestamp:f=void 0}=i,{largeImageKey:d=void 0}=i,{largeImageText:s=void 0}=i,{smallImageKey:u=void 0}=i,{smallImageText:g=void 0}=i,{partyId:r=void 0}=i,{partySize:y=void 0}=i,{partyMax:I=void 0}=i,{matchSecret:_=void 0}=i,{joinSecret:S=void 0}=i,{spectateSecret:h=void 0}=i,{instance:T=!1}=i,{buttons:x=void 0}=i,{page:b=""}=i,{video:N={}}=i,{other:o={}}=i;const K={details:a,state:l,startTimestamp:c,endTimestamp:f,largeImageKey:d,largeImageText:s,smallImageKey:u,smallImageText:g,partyId:r,partySize:y,partyMax:I,matchSecret:_,joinSecret:S,spectateSecret:h,instance:T,buttons:x},M={page:b,video:N,other:o};return E(()=>{if(!n)return;["true",null].includes(localStorage.getItem("settings.discordRPCActive"))?(window.NativeApp.startRPC(),window.NativeApp.updateRPC(K)):window.NativeApp.stopRPC()}),m.$$set=e=>{"details"in e&&t(3,a=e.details),"state"in e&&t(4,l=e.state),"startTimestamp"in e&&t(5,c=e.startTimestamp),"endTimestamp"in e&&t(6,f=e.endTimestamp),"largeImageKey"in e&&t(7,d=e.largeImageKey),"largeImageText"in e&&t(8,s=e.largeImageText),"smallImageKey"in e&&t(9,u=e.smallImageKey),"smallImageText"in e&&t(10,g=e.smallImageText),"partyId"in e&&t(11,r=e.partyId),"partySize"in e&&t(12,y=e.partySize),"partyMax"in e&&t(13,I=e.partyMax),"matchSecret"in e&&t(14,_=e.matchSecret),"joinSecret"in e&&t(15,S=e.joinSecret),"spectateSecret"in e&&t(16,h=e.spectateSecret),"instance"in e&&t(17,T=e.instance),"buttons"in e&&t(18,x=e.buttons),"page"in e&&t(19,b=e.page),"video"in e&&t(20,N=e.video),"other"in e&&t(21,o=e.other)},[n,K,M,a,l,c,f,d,s,u,g,r,y,I,_,S,h,T,x,b,N,o]}class Y extends B{constructor(i){super(),G(this,i,Q,L,j,{details:3,state:4,startTimestamp:5,endTimestamp:6,largeImageKey:7,largeImageText:8,smallImageKey:9,smallImageText:10,partyId:11,partySize:12,partyMax:13,matchSecret:14,joinSecret:15,spectateSecret:16,instance:17,buttons:18,page:19,video:20,other:21})}}export{Y as D};
