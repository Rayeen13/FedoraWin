(()=> {
  const root=document.documentElement;
  const key='fedorawin-docs-theme';
  const stored=localStorage.getItem(key);
  const preferred=matchMedia('(prefers-color-scheme: light)').matches?'light':'dark';
  root.dataset.theme=stored||preferred;

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

  const lightbox=document.querySelector('#lightbox');
  if(lightbox){
    const image=lightbox.querySelector('img');
    const caption=lightbox.querySelector('figcaption');
    const close=()=> {
      lightbox.classList.remove('open');
      lightbox.setAttribute('aria-hidden','true');
      image?.removeAttribute('src');
    };
    document.querySelectorAll('[data-image]').forEach(el=>el.addEventListener('click',()=>{
      if(image) image.src=el.dataset.image;
      if(caption) caption.textContent=el.dataset.caption||'';
      lightbox.classList.add('open');
      lightbox.setAttribute('aria-hidden','false');
      lightbox.querySelector('button')?.focus();
    }));
    lightbox.querySelector('button')?.addEventListener('click',close);
    lightbox.addEventListener('click',event=>{if(event.target===lightbox) close()});
    addEventListener('keydown',event=>{
      if(event.key==='Escape'){close();closeMenu();}
    });
  }else{
    addEventListener('keydown',event=>{if(event.key==='Escape') closeMenu()});
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

      document.querySelectorAll('[data-ci-provenance]').forEach(el=>{
        el.textContent='Rendered from '+metadata.source_branch+' @ '+shortSha+' · '+capturedLabel+' · '+image+' · '+resolution;
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