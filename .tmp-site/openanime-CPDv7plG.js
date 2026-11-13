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

function l(t,e){return Math.floor(Math.random()*(e-t+1)+t)}function c(t){return t[l(0,t.length-1)]}function s(t,e=0){const o="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";let n="";for(let r=0;r<t;r++)n+=o.charAt(Math.floor(Math.random()*o.length));for(let r=0;r<e;r++){const a=l(1,n.length-2);n=n.slice(0,a)+`
`+n.slice(a)}return n}export{c as a,s as b,l as r};
