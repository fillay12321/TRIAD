import React, { useEffect, useRef } from 'react';
import './Innovations.css';

export type Innovation = {
  title: string;
  description: string;
};

export default function Innovations({ items }: { items: Innovation[] }) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    let ctx: any;
    let alive = true;
    (async () => {
      const gsapMod = await import('gsap');
      const gsap = gsapMod.default || gsapMod;
      const stMod = await import('gsap/ScrollTrigger');
      const ScrollTrigger = (stMod as any).default || (stMod as any).ScrollTrigger;
      gsap.registerPlugin(ScrollTrigger);

      if (!alive) return;
      ctx = gsap.context(() => {
        const sections = gsap.utils.toArray<HTMLDivElement>('.innovation-section');

        sections.forEach((sec) => {
          const heading = sec.querySelector('.innovation-title');
          const text = sec.querySelector('.innovation-text');

          gsap.set([heading, text], { opacity: 0, y: 60, willChange: 'transform, opacity' });

          gsap.timeline({
            scrollTrigger: {
              trigger: sec,
              start: 'top 75%',
              end: '+=60% bottom',
              scrub: false,
              once: true,
            },
          })
            .to(heading, { opacity: 1, y: 0, duration: 0.8, ease: 'power3.out' })
            .to(text, { opacity: 1, y: 0, duration: 0.8, ease: 'power3.out' }, '-=0.35');

          // Parallax subtle shift
          gsap.to(sec, {
            y: 0,
            scrollTrigger: {
              trigger: sec,
              start: 'top bottom',
              end: 'bottom top',
              scrub: true,
            },
          });
        });
      }, containerRef);
    })();

    return () => {
      alive = false;
      if (ctx) ctx.revert();
    };
  }, []);

  return (
    <div ref={containerRef} className="innovations-container">
      {items.map((it, idx) => (
        <section className="innovation-section" key={idx}>
          <h2 className="innovation-title">{it.title}</h2>
          <p className="innovation-text">{it.description}</p>
        </section>
      ))}
    </div>
  );
}
