import { Component, inject } from '@angular/core';
import { RouterLink } from '@angular/router';
import { AuthService } from '@attome/base';

@Component({
  selector:   'app-dashboard',
  standalone: true,
  imports:    [RouterLink],
  template: `
    <div class="page-header">
      <h2>Dashboard</h2>
      <p class="subtitle">Welcome back, {{ shortId() }}</p>
    </div>

    <div class="cards">
      <div class="card">
        <div class="card-icon">◫</div>
        <h3>Entities</h3>
        <p>Browse and manage your XRM entity records.</p>
        <a routerLink="/xrm/entity/contact" class="card-link">Open Contacts →</a>
      </div>
      <div class="card">
        <div class="card-icon">⚙</div>
        <h3>Settings</h3>
        <p>Configure your tenant and platform settings.</p>
        <span class="card-link muted">Coming soon</span>
      </div>
      <div class="card">
        <div class="card-icon">◈</div>
        <h3>Workflows</h3>
        <p>Automate processes with the workflow engine.</p>
        <span class="card-link muted">Coming soon</span>
      </div>
    </div>
  `,
  styles: [`
    .page-header  { margin-bottom: 32px; }
    .page-header h2 { font-size: 24px; font-weight: 700; color: #111827; }
    .subtitle     { color: #6b7280; margin-top: 4px; }
    .cards        { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }
    .card         { background: #fff; border-radius: 12px; padding: 24px;
                    border: 1px solid #e5e7eb; }
    .card-icon    { font-size: 28px; margin-bottom: 12px; }
    .card h3      { font-size: 16px; font-weight: 600; margin-bottom: 6px; }
    .card p       { font-size: 13px; color: #6b7280; line-height: 1.6; margin-bottom: 16px; }
    .card-link    { font-size: 13px; font-weight: 500; color: #6366f1; cursor: pointer; }
    .card-link.muted { color: #9ca3af; cursor: default; }
  `],
})
export class DashboardComponent {
  private readonly auth = inject(AuthService);
  shortId() { return (this.auth.userId() ?? '').slice(0, 8); }
}
