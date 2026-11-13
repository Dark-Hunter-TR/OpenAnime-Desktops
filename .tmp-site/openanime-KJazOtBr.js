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

import{e as o}from"./openanime-LfDqg_SJ.js";async function r({slugs:t,seasons:e,episodes:i}){return await fetch(`${o.PUBLIC_API_LINK}/user/activity/${t.join(",")}/${e.join(",")}/${i.join(",")}`,{method:"DELETE",headers:{Authorization:window.getCookie("token")}}),!0}export{r as d};
