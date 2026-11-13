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

function v(u){const c=new Date(u),i=new Date-c,e=Math.floor(i/1e3),t=Math.floor(e/60),o=Math.floor(t/60),s=Math.floor(o/24),n=Math.floor(s/7),r=Math.floor(n/4),a=Math.floor(r/12),m=e===1?"second":"seconds",l=t===1?"minute":"minutes",f=o===1?"hour":"hours",y=s===1?"day":"days",h=n===1?"week":"weeks",k=r===1?"month":"months",d=a===1?"year":"years";return a>0?{key:`time.${d}Ago`,value:a}:r>0?{key:`time.${k}Ago`,value:r}:n>0?{key:`time.${h}Ago`,value:n}:s>0?{key:`time.${y}Ago`,value:s}:o>0?{key:`time.${f}Ago`,value:o}:t>0?{key:`time.${l}Ago`,value:t}:e===0?{key:"time.now"}:{key:`time.${m}Ago`,value:e}}export{v as t};
