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
})();