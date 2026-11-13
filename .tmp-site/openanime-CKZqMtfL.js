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

import{g as l}from"./openanime-sATobStx.js";import{e as u}from"./openanime-BW76-m6_.js";import{P as y}from"./openanime-BW76-m6_.js";import{p as r,e as c}from"./openanime-LfDqg_SJ.js";async function f({fetch:s,params:o}){o.slug;let t={};{const e=l(r);e!=null&&e.id?(r.set({}),t=e):i()}async function i(e=!1){async function n(){const a=await s(`${c.PUBLIC_API_LINK}/anime/${o.slug}`,{headers:{"Client-Protocol-Model":"RCSA-14402/05"}});return a.status==404?u(404,{message:"Not found"}):await a.json()}t=e?await n():n()}return{anime:t}}const g=Object.freeze(Object.defineProperty({__proto__:null,load:f},Symbol.toStringTag,{value:"Module"}));export{y as component,g as universal};
