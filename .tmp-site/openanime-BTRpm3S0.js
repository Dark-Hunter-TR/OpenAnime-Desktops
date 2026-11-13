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

function b(u,s){const e=s.trim().toLocaleLowerCase("tr");if(!e)return[...u];const t=e.match(/^(\d+)\.?\s*(?:bölüm)?$|^bölüm\s+(\d+)\.?$/),m=e.match(/^(\d+)\s*-\s*(\d+)$/),a=e.match(/^f[ıi]ller(s[ıi]z)?$/);return u.filter(r=>{var n,c;if(t)return Number(r.episodeNumber)===Number(t[1]??t[2]);if(m)return Number(r.episodeNumber)>=Number(m[1])&&Number(r.episodeNumber)<=Number(m[2]);if(a)return a[1]?!r.filler:!!r.filler;const l=`bölüm ${r.episodeNumber} - `+(((n=r.name)==null?void 0:n.toLocaleLowerCase("tr"))||""),o=((c=r.summary)==null?void 0:c.toLocaleLowerCase("tr"))||"";return l.includes(e)||o.includes(e)})}export{b as s};
