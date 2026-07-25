document.addEventListener('DOMContentLoaded', () => {
    const modal = document.createElement('div');
    modal.id = 'cws-lightbox';
    modal.className = 'fixed inset-0 z-[100] bg-slate-900/95 hidden flex justify-center items-center backdrop-blur-sm cursor-zoom-out opacity-0 transition-opacity duration-300';
    
    const imgWrapper = document.createElement('div');
    imgWrapper.className = 'relative max-w-[95vw] max-h-[95vh] transition-transform duration-300 scale-95';
    
    const modalImg = document.createElement('img');
    modalImg.className = 'max-w-full max-h-[95vh] object-contain rounded-lg shadow-2xl';
    
    const closeBtn = document.createElement('button');
    closeBtn.className = 'absolute -top-12 right-0 text-white hover:text-indigo-400 transition-colors';
    closeBtn.innerHTML = '<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"></path></svg>';

    imgWrapper.appendChild(modalImg);
    imgWrapper.appendChild(closeBtn);
    modal.appendChild(imgWrapper);
    document.body.appendChild(modal);

    const resolveZoomSource = (img) => {
        if (img.dataset.lightboxSrc) return img.dataset.lightboxSrc;
        if (img.currentSrc) return img.currentSrc;
        const pictureSource = img.closest('picture')?.querySelector('source[srcset]');
        if (pictureSource) {
            const srcset = pictureSource.getAttribute('srcset') || '';
            const firstCandidate = srcset.split(',')[0]?.trim().split(/\s+/)[0];
            if (firstCandidate) return firstCandidate;
        }
        return img.src;
    };

    const openModal = (src, alt) => {
        modalImg.src = src;
        modalImg.alt = alt || '';
        modal.classList.remove('hidden');
        requestAnimationFrame(() => {
            modal.classList.remove('opacity-0');
            imgWrapper.classList.remove('scale-95');
        });
        document.body.style.overflow = 'hidden';
    };

    const closeModal = () => {
        modal.classList.add('opacity-0');
        imgWrapper.classList.add('scale-95');
        setTimeout(() => {
            modal.classList.add('hidden');
            modalImg.src = '';
            modalImg.alt = '';
            document.body.style.overflow = '';
        }, 300);
    };

    modal.addEventListener('click', (e) => {
        if (e.target === modal || e.target === closeBtn || e.target.closest('button') === closeBtn || e.target === modalImg) {
            closeModal();
        }
    });

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !modal.classList.contains('hidden')) {
            closeModal();
        }
    });

    const selector = 'main img:not(.no-zoom):not([src*="logo"]), article img:not(.no-zoom)';
    
    document.querySelectorAll(selector).forEach(img => {
        if (img.clientWidth > 100) {
            img.style.cursor = 'zoom-in';
            img.classList.add('transition-transform', 'duration-300', 'hover:scale-[1.01]');
            
            img.addEventListener('click', (e) => {
                e.preventDefault();
                openModal(resolveZoomSource(img), img.alt);
            });
        }
    });
});
