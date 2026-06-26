import { Component, inject } from '@angular/core';
import { RouterLink } from '@angular/router';
import { AuthModalService } from '../../services/auth-modal.service';

interface Faq {
  q: string;
  a: string;
  open: boolean;
}

@Component({
  selector: 'app-landing',
  standalone: true,
  imports: [RouterLink],
  templateUrl: './landing.component.html',
  styleUrl: './landing.component.css',
})
export class LandingComponent {
  private readonly authModal = inject(AuthModalService);

  openSignIn(): void { this.authModal.open(); }

  faqs: Faq[] = [
    {
      q: 'What exactly is an XRM platform?',
      a: 'Extended Relationship Management — a configurable data and process engine that adapts to any domain. Unlike rigid CRMs or ERPs, Attome\'s XRM lets you model any entity, workflow, or relationship: case management, licensing, citizen services, asset tracking, and more — all without writing code.',
      open: false,
    },
    {
      q: 'Can it be deployed on-premise or in an air-gapped environment?',
      a: 'Yes. Attome ships as a single static Rust binary with no external dependencies. The same binary powers SaaS deployments on Kubernetes and classified ministry installations with no internet connection — identical feature set, zero configuration delta.',
      open: false,
    },
    {
      q: 'Is Arabic language truly first-class, or just translated?',
      a: 'Arabic is native, not translated. The entire platform is RTL-first at the framework level — including Hijri calendar, Arabic numerals, GCC date formats, and bidirectional text handling. There is no English-primary default that Arabic is patched over.',
      open: false,
    },
    {
      q: 'How does the sovereign AI work without sending data outside?',
      a: 'Embeddings and inference run in-process on your own infrastructure using the built-in multi-LLM gateway. You can connect OpenAI, Anthropic, Google AI, DeepSeek, or a locally-hosted model — and the RAG pipeline never routes data through third-party servers.',
      open: false,
    },
  ];

  toggle(faq: Faq): void {
    faq.open = !faq.open;
  }
}
