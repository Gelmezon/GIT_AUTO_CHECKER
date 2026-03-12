import type { AuthResponse, User } from '../types'
import { request } from './client'

export function login(email: string, password: string) {
  return request<AuthResponse>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
}

export function activate(email: string, password: string) {
  return request<AuthResponse>('/api/auth/activate', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
}

export function fetchMe() {
  return request<User>('/api/me')
}
