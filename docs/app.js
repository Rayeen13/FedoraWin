(()=> {
  const root=document.documentElement;
  const key='fedorawin-docs-theme';
  const stored=localStorage.getItem(key);
  const preferred=matchMedia('(prefers-color-scheme: light)').matches?'light':'dark';
  root.dataset.theme=stored||preferred;

  const ensureDocsLink=()=> {
    const insert=(nav,mobile=false)=> {
      if(!nav||nav.querySelector('a[href="./docs.html"]')) return;
      const link=document.createElement('a');
      link.href='./docs.html';
      link.textContent='Docs';
      if(location.pathname.endsWith('/docs.html')) link.setAttribute('aria-current','page');
      const before=nav.querySelector('a[href="./getting-started.html"]');
      if(before) nav.insertBefore(link,before); else nav.append(link);
    };
    insert(document.querySelector('.links'));
    insert(document.querySelector('#mobileNav'),true);
  };
  ensureDocsLink();

  // GNOME-inspired site navigation. This is website chrome, not an emulated
  // system panel or evidence of a shipped FedoraWin desktop capability.
  const sitePanel=document.createElement('div');
  sitePanel.className='site-panel';
  sitePanel.innerHTML=[
    '<div class="site-panel__inside">',
    '<button type="button" class="site-panel__activities" id="siteActivities" aria-controls="siteOverview" aria-expanded="false"><span class="site-panel__dot" aria-hidden="true"></span> Activities</button>',
    '<time class="site-panel__clock" id="siteClock" aria-label="Your local date and time"></time>',
    '<span class="site-panel__edition"><span class="site-panel__edition-dot" aria-hidden="true"></span> FedoraWin <span class="site-panel__edition-detail">· website preview</span></span>',
    '</div>'
  ].join('');
  document.body.prepend(sitePanel);
  const siteOverview=document.createElement('div');
  siteOverview.className='site-overview';
  siteOverview.id='siteOverview';
  siteOverview.hidden=true;
  siteOverview.setAttribute('role','dialog');
  siteOverview.setAttribute('aria-modal','true');
  siteOverview.setAttribute('aria-label','FedoraWin website Activities');
  siteOverview.innerHTML=[
    '<div class="site-overview__backdrop" data-close-overview></div>',
    '<div class="site-overview__content">',
      '<div class="site-overview__header"><div class="site-overview__heading"><img src="./assets/fedorawin-logo.svg" width="44" height="44" alt=""><div><strong>Activities</strong><span>Explore FedoraWin · website navigation</span></div></div><button class="site-overview__close" type="button" data-close-overview aria-label="Close Activities">×</button></div>',
      '<div class="site-overview__intro">Your FedoraWin workspace <span>— GNOME 51 design target · pre-beta</span></div>',
      '<div class="site-overview__tiles">',
        '<a href="./gallery.html" class="site-overview__tile site-overview__tile--large"><img src="./assets/runtime/activities.png" alt="Actual FedoraWin.exe Activities screenshot"><span><b>Gallery</b><small>Browse real Windows CI captures ↗</small></span></a>',
        '<a href="./status.html" class="site-overview__tile"><span class="site-overview__glyph" aria-hidden="true">◉</span><span><b>Development status</b><small>Verified progress & beta gates ↗</small></span></a>',
        '<a href="./docs.html" class="site-overview__tile"><span class="site-overview__glyph" aria-hidden="true">▤</span><span><b>Documentation</b><small>Explore the implementation ↗</small></span></a>',
        '<a href="./architecture.html" class="site-overview__tile"><span class="site-overview__glyph" aria-hidden="true">⌘</span><span><b>Architecture</b><small>Windows underneath ↗</small></span></a>',
        '<a href="./getting-started.html" class="site-overview__tile"><span class="site-overview__glyph" aria-hidden="true">↗</span><span><b>Test FedoraWin</b><small>Pre-beta development build ↗</small></span></a>',
      '</div>',
      '<p class="site-overview__foot">This overview navigates the website; it does not control your Windows desktop. <span>Esc to close</span></p>',
    '</div>'
  ].join('');
  document.body.append(siteOverview);
  const activitiesButton=sitePanel.querySelector('#siteActivities');
  const overviewClose=siteOverview.querySelector('.site-overview__close');
  let overviewReturnFocus=null;
  const closeSiteOverview=()=>{
    if(siteOverview.hidden) return;
    siteOverview.classList.remove('is-open');
    siteOverview.hidden=true;
    document.body.classList.remove('site-overview-open');
    activitiesButton.setAttribute('aria-expanded','false');
    overviewReturnFocus?.focus({preventScroll:true});
    overviewReturnFocus=null;
  };
  const openSiteOverview=()=>{
    if(!siteOverview.hidden){closeSiteOverview();return;}
    overviewReturnFocus=document.activeElement;
    siteOverview.hidden=false;
    document.body.classList.add('site-overview-open');
    activitiesButton.setAttribute('aria-expanded','true');
    requestAnimationFrame(()=>siteOverview.classList.add('is-open'));
    overviewClose.focus({preventScroll:true});
  };
  activitiesButton.addEventListener('click',openSiteOverview);
  siteOverview.querySelectorAll('[data-close-overview]').forEach(node=>node.addEventListener('click',closeSiteOverview));
  siteOverview.addEventListener('keydown',event=>{
    if(event.key==='Escape'){event.preventDefault();closeSiteOverview();return;}
    if(event.key!=='Tab') return;
    const focusables=[...siteOverview.querySelectorAll('button,a[href]')].filter(el=>!el.hidden);
    if(!focusables.length) return;
    if(event.shiftKey&&document.activeElement===focusables[0]){event.preventDefault();focusables.at(-1).focus();}
    else if(!event.shiftKey&&document.activeElement===focusables.at(-1)){event.preventDefault();focusables[0].focus();}
  });
  const siteClock=sitePanel.querySelector('#siteClock');
  const updateSiteClock=()=>{
    const now=new Date();
    siteClock.dateTime=now.toISOString();
    siteClock.textContent=now.toLocaleString(undefined,{weekday:'short',hour:'numeric',minute:'2-digit'});
    siteClock.title=now.toLocaleString(undefined,{dateStyle:'full',timeStyle:'short'});
  };
  updateSiteClock();
  setInterval(updateSiteClock,60000);

  // A thin scroll indicator deliberately follows the page, not the mouse.
  const pageProgress=document.createElement('div');
  pageProgress.className='site-scroll-progress';
  pageProgress.setAttribute('aria-hidden','true');
  document.body.append(pageProgress);
  let scrollQueued=false;
  const updatePageProgress=()=>{
    scrollQueued=false;
    const total=document.documentElement.scrollHeight-window.innerHeight;
    pageProgress.style.transform='scaleX('+(total>0?Math.min(1,Math.max(0,window.scrollY/total)):0)+')';
  };
  addEventListener('scroll',()=>{
    if(scrollQueued) return;
    scrollQueued=true;
    requestAnimationFrame(updatePageProgress);
  },{passive:true});
  addEventListener('resize',updatePageProgress,{passive:true});
  updatePageProgress();


  const theme=document.querySelector('#theme');
  const syncThemeLabel=()=> {
    if(!theme) return;
    const next=root.dataset.theme==='light'?'dark':'light';
    theme.setAttribute('aria-label','Switch to '+next+' theme');
    theme.title='Switch to '+next+' theme';
  };
  syncThemeLabel();
  theme?.addEventListener('click',()=> {
    const next=root.dataset.theme==='light'?'dark':'light';
    root.dataset.theme=next;
    localStorage.setItem(key,next);
    syncThemeLabel();
  });

  const menu=document.querySelector('#menu');
  const mobile=document.querySelector('#mobileNav');
  const closeMenu=()=> {
    mobile?.classList.remove('open');
    menu?.setAttribute('aria-expanded','false');
  };
  menu?.addEventListener('click',()=> {
    const open=mobile?.classList.toggle('open')||false;
    menu.setAttribute('aria-expanded',String(open));
  });
  mobile?.querySelectorAll('a').forEach(a=>a.addEventListener('click',closeMenu));

  if('IntersectionObserver' in window){
    const observer=new IntersectionObserver(entries=>entries.forEach(entry=>{
      if(entry.isIntersecting){
        entry.target.classList.add('visible');
        observer.unobserve(entry.target);
      }
    }),{threshold:.12});
    document.querySelectorAll('.reveal').forEach(el=>observer.observe(el));
  }else{
    document.querySelectorAll('.reveal').forEach(el=>el.classList.add('visible'));
  }

  // Treat every real executable capture as an ordered album. The source buttons
  // are never cloned: runtime CI hydration updates their data-image URLs in place.
  const lightbox=document.querySelector('#lightbox');
  if(lightbox){
    const entries=[...document.querySelectorAll('[data-image]')];
    const image=lightbox.querySelector('img');
    const caption=lightbox.querySelector('figcaption');
    const closeButton=lightbox.querySelector(':scope > button');
    const figure=lightbox.querySelector('figure');
    const createButton=(className,label,text)=>{
      const button=document.createElement('button');
      button.type='button';
      button.className=className;
      button.setAttribute('aria-label',label);
      button.textContent=text;
      return button;
    };
    const previous=createButton('lightbox-nav lightbox-previous','Previous image','←');
    const next=createButton('lightbox-nav lightbox-next','Next image','→');
    const position=document.createElement('output');
    position.className='lightbox-position';
    position.setAttribute('aria-live','polite');
    figure?.append(position);
    figure?.before(previous);
    figure?.after(next);
    closeButton?.classList.add('lightbox-close');
    closeButton?.setAttribute('aria-label','Close image viewer');
    lightbox.setAttribute('role','dialog');
    lightbox.setAttribute('aria-modal','true');
    lightbox.setAttribute('aria-label','FedoraWin screenshot viewer');
    lightbox.inert=true;
    let active=-1;
    let opener=null;
    let touchX=null;
    const isOpen=()=>lightbox.classList.contains('open');
    const show=index=>{
      if(!entries.length) return;
      active=(index+entries.length)%entries.length;
      const entry=entries[active];
      if(image){
        image.src=entry.dataset.image;
        image.alt=entry.querySelector('img')?.alt||entry.dataset.caption||'FedoraWin screenshot';
      }
      if(caption) caption.textContent=entry.dataset.caption||entry.querySelector('span b')?.textContent||'FedoraWin';
      position.textContent=(active+1)+' / '+entries.length;
      // Keep the single-card carousel in sync with the opened album image.
      document.dispatchEvent(new CustomEvent('fedorawin:gallery-image-selected',{detail:{entry}}));
    };
    const close=()=>{
      if(!isOpen()) return;
      lightbox.classList.remove('open');
      lightbox.setAttribute('aria-hidden','true');
      document.body.classList.remove('viewer-open');
      image?.removeAttribute('src');
      if(opener?.hidden){
        // Album navigation may hide the originally opened carousel card.
        document.querySelector('#galleryGrid .gallery-card:not([hidden])')?.focus({preventScroll:true});
      }else{
        opener?.focus({preventScroll:true});
      }
      lightbox.inert=true;
      active=-1;
    };
    entries.forEach((entry,index)=>entry.addEventListener('click',()=>{
      opener=entry;
      show(index);
      lightbox.inert=false;
      lightbox.classList.add('open');
      lightbox.setAttribute('aria-hidden','false');
      document.body.classList.add('viewer-open');
      closeButton?.focus({preventScroll:true});
    }));
    closeButton?.addEventListener('click',close);
    previous.addEventListener('click',()=>show(active-1));
    next.addEventListener('click',()=>show(active+1));
    lightbox.addEventListener('click',event=>{if(event.target===lightbox) close()});
    lightbox.addEventListener('touchstart',event=>{
      touchX=isOpen()&&event.touches.length===1?event.touches[0].clientX:null;
    },{passive:true});
    lightbox.addEventListener('touchend',event=>{
      if(touchX===null||!isOpen()) return;
      const delta=event.changedTouches[0].clientX-touchX;
      touchX=null;
      if(Math.abs(delta)>48) show(active+(delta<0?1:-1));
    },{passive:true});
    addEventListener('keydown',event=>{
      if(event.key==='Escape'){close();closeMenu();return;}
      if(!isOpen()) return;
      if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){
        event.preventDefault();
        if(event.key==='Home') show(0);
        else if(event.key==='End') show(entries.length-1);
        else show(active+(event.key==='ArrowRight'?1:-1));
      }
      if(event.key==='Tab'){
        const order=[closeButton,previous,next].filter(Boolean);
        if(event.shiftKey&&document.activeElement===order[0]){event.preventDefault();order.at(-1).focus();}
        else if(!event.shiftKey&&document.activeElement===order.at(-1)){event.preventDefault();order[0].focus();}
      }
    });
  }else{
    addEventListener('keydown',event=>{if(event.key==='Escape') closeMenu()});
  }

  // All four layouts reuse the same screenshot buttons, captions and CI hydration.
  // No cloned or mock images: the live runtime metadata updates one source of truth.
  const gallery=document.querySelector('#galleryGrid');
  if(gallery){
    const cards=[...gallery.querySelectorAll('[data-runtime-shot]')];
    const modes=new Set(['list','grid','card','carousel']);
    const toggles=[...document.querySelectorAll('[data-gallery-view]')];
    const controls=document.querySelector('#galleryCarouselControls');
    const position=document.querySelector('#galleryPosition');
    const previous=document.querySelector('#galleryPrevious');
    const next=document.querySelector('#galleryNext');
    const viewKey='fedorawin-gallery-view';
    let active=0;
    let touchX=null;
    let view='card';
    try{
      const stored=localStorage.getItem(viewKey);
      if(modes.has(stored)) view=stored;
    }catch(_){ /* A private browser can deny persistence. */ }

    const render=()=>{
      gallery.dataset.view=view;
      const carousel=view==='carousel';
      if(controls) controls.hidden=!carousel;
      toggles.forEach(button=>{
        const selected=button.dataset.galleryView===view;
        button.setAttribute('aria-pressed',String(selected));
        button.classList.toggle('is-active',selected);
      });
      cards.forEach((card,index)=>{
        card.hidden=carousel&&index!==active;
        // Hidden cards were not eligible for IntersectionObserver. Reveal them
        // when switching back so the gallery never appears empty.
        if(carousel||view!=='carousel') card.classList.add('visible');
        card.setAttribute('aria-label',(index+1)+' of '+cards.length+': '+(card.querySelector('span b')?.textContent||'Screenshot'));
      });
      if(position){
        position.textContent=(active+1)+' / '+cards.length+' · '+(cards[active]?.querySelector('span b')?.textContent||'');
      }
    };
    const select=(index,focusCard=false)=>{
      if(!cards.length) return;
      active=(index+cards.length)%cards.length;
      render();
      if(focusCard) cards[active].focus({preventScroll:true});
    };
    toggles.forEach(button=>button.addEventListener('click',()=>{
      const choice=button.dataset.galleryView;
      if(!modes.has(choice)) return;
      view=choice;
      try{localStorage.setItem(viewKey,view)}catch(_){}
      render();
    }));
    document.addEventListener('fedorawin:gallery-image-selected',event=>{
      const index=cards.indexOf(event.detail.entry);
      if(index>=0) select(index);
    });
    previous?.addEventListener('click',()=>select(active-1));
    next?.addEventListener('click',()=>select(active+1));
    gallery.addEventListener('keydown',event=>{
      if(view!=='carousel'||!['ArrowLeft','ArrowRight'].includes(event.key)) return;
      if(document.querySelector('#lightbox.open')) return;
      event.preventDefault();
      select(active+(event.key==='ArrowRight'?1:-1),true);
    });
    gallery.addEventListener('touchstart',event=>{
      touchX=view==='carousel'&&event.touches.length===1?event.touches[0].clientX:null;
    },{passive:true});
    gallery.addEventListener('touchend',event=>{
      if(touchX===null||view!=='carousel') return;
      const change=event.changedTouches[0].clientX-touchX;
      touchX=null;
      if(Math.abs(change)>48) select(active+(change<0?1:-1));
    },{passive:true});
    render();
  }

  const hydrateCiPreview=async()=> {
    try{
      const response=await fetch('./assets/runtime/preview-metadata.json',{cache:'no-store'});
      if(!response.ok) return;
      const metadata=await response.json();
      const shots=metadata.screenshots||{};

      document.querySelectorAll('[data-runtime-shot]').forEach(el=>{
        const file=shots[el.dataset.runtimeShot];
        if(!file) return;
        const src='./assets/runtime/'+file;
        if(el.tagName==='IMG'){
          el.src=src;
        }else{
          const img=el.querySelector('img');
          if(img) img.src=src;
          if(el.hasAttribute('data-image')) el.dataset.image=src;
        }
      });

      const sha=String(metadata.source_sha||'').trim();
      const shortSha=sha?sha.slice(0,10):'unknown';
      const run=String(metadata.github_run_number||'').trim();
      const runId=String(metadata.github_run_id||'').trim();
      const captured=metadata.captured_utc?new Date(metadata.captured_utc):null;
      const capturedLabel=captured&&!Number.isNaN(captured.valueOf())
        ? captured.toLocaleString(undefined,{dateStyle:'medium',timeStyle:'short'})
        : 'latest successful capture';
      const resolution=metadata.screen&&metadata.screen.width&&metadata.screen.height
        ? metadata.screen.width+'×'+metadata.screen.height
        : 'runner desktop';
      const image=String(metadata.runner_image||'Windows runner');
      const runtime=String(metadata.runtime||'FedoraWin.exe');

      document.querySelectorAll('[data-ci-provenance]').forEach(el=>{
        el.textContent=runtime+' · '+metadata.source_branch+' @ '+shortSha+' · '+capturedLabel+' · '+image+' · '+resolution;
        if(runId){
          el.append(' · ');
          const link=document.createElement('a');
          link.href='https://github.com/Rayeen13/FedoraWin/actions/runs/'+runId;
          link.target='_blank';
          link.rel='noreferrer';
          link.textContent=run?'GitHub Actions #'+run:'GitHub Actions run';
          el.append(link);
        }
      });

      document.querySelectorAll('[data-ci-run-link]').forEach(link=>{
        if(runId) link.href='https://github.com/Rayeen13/FedoraWin/actions/runs/'+runId;
      });
      root.classList.add('ci-preview-ready');
    }catch(_){
      // Keep committed screenshots and static copy as a reliable fallback.
    }
  };
  hydrateCiPreview();

})();