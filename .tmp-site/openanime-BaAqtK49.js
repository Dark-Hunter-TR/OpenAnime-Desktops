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

class u extends EventTarget{constructor(e){super(),this.ready=!1;const t=indexedDB.open(e,2);t.onupgradeneeded=n=>{this.db=n.target.result,this.db.objectStoreNames.contains("new-infra-videos")||this.db.createObjectStore("new-infra-videos",{keyPath:"videoId"}),this.db.objectStoreNames.contains("new-infra-videos-subtitles")||this.db.createObjectStore("new-infra-videos-subtitles",{keyPath:"videoId"}),this.db.objectStoreNames.contains("new-infra-images")||this.db.createObjectStore("new-infra-images",{keyPath:"imageId"})},t.onsuccess=n=>{this.db=n.target.result,this.ready=!0;const s=new CustomEvent("ready");this.dispatchEvent(s)}}getVideo(e){return new Promise(t=>{const o=this.db.transaction("new-infra-videos","readonly").objectStore("new-infra-videos").get(e);o.onsuccess=function(r){const i=r.target.result;t(i||!1)},o.onerror=function(){t(!1)}})}addVideo(e,t){return new Promise(n=>{const r=this.db.transaction("new-infra-videos","readwrite").objectStore("new-infra-videos").add({mp4File:e,videoId:t});r.onsuccess=function(){n(1)},r.onerror=function(){n(!1)}})}deleteVideo(e){return new Promise(t=>{const o=this.db.transaction("new-infra-videos","readwrite").objectStore("new-infra-videos").delete(e);o.onsuccess=function(){t(1)},o.onerror=function(){t(!1)}})}getSubtitle(e){return new Promise(t=>{const o=this.db.transaction("new-infra-videos-subtitles","readonly").objectStore("new-infra-videos-subtitles").get(e);o.onsuccess=function(r){const i=r.target.result;t(i||!1)},o.onerror=function(){t(!1)}})}addSubtitle(e,t){return new Promise(n=>{const r=this.db.transaction("new-infra-videos-subtitles","readwrite").objectStore("new-infra-videos-subtitles").add({videoId:e,data:t});r.onsuccess=function(){n(1)},r.onerror=function(){n(!1)}})}deleteSubtitle(e){return new Promise(t=>{const o=this.db.transaction("new-infra-videos-subtitles","readwrite").objectStore("new-infra-videos-subtitles").delete(e);o.onsuccess=function(){t(1)},o.onerror=function(){t(!1)}})}getImage(e,t="avatar"){const n=`${e}-${t}`;return new Promise(s=>{const i=this.db.transaction("new-infra-images","readonly").objectStore("new-infra-images").get(n);i.onsuccess=function(a){const c=a.target.result;c&&c.imageBlob?s(c.imageBlob):s(!1)},i.onerror=function(){s(!1)}})}addImage(e,t,n){const s=`${e}-${t}`;return new Promise(o=>{const a=this.db.transaction("new-infra-images","readwrite").objectStore("new-infra-images").put({imageId:s,imageBlob:n});a.onsuccess=function(){o(1)},a.onerror=function(){o(!1)}})}}export{u as V};
