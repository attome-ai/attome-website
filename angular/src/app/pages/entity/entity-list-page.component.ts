import { Component, inject, input } from '@angular/core';
import { Router } from '@angular/router';
import { EntityListComponent } from '@attome/xrm';
import { EntityRecord } from '@attome/xrm';

@Component({
  selector:   'app-entity-list-page',
  standalone: true,
  imports:    [EntityListComponent],
  template: `
    <xrm-entity-list
      [entityName]="entityName()"
      (newClick)="onCreate()"
      (editClick)="onEdit($event)"
    />
  `,
})
export class EntityListPageComponent {
  private readonly router = inject(Router);

  readonly entityName = input('', { alias: 'name' });

  onCreate(): void {
    this.router.navigate(['/entity', this.entityName(), 'new']);
  }

  onEdit(record: EntityRecord): void {
    this.router.navigate(['/entity', this.entityName(), record.id, 'edit']);
  }
}
