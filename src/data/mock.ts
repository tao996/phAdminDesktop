export type ServerStatus = "connected" | "checking" | "offline";

export interface Server {
  id: string;
  name: string;
  host: string;
  environment: "production" | "staging" | "local";
  status: ServerStatus;
  latency: string;
  kind?: string;
  port?: number;
  username?: string;
}

export type ProjectStatus = "running" | "stopped" | "not-deployed" | "warning";

export interface Container {
  service: string;
  image: string;
  status: "running" | "stopped";
  port: string;
}

export interface Project {
  id: string;
  serverId: string;
  name: string;
  title: string;
  path: string;
  domain: string;
  port: number;
  branch: string;
  status: ProjectStatus;
  ssl: boolean;
  lastDeploy: string;
  backups: number;
  phpVersion: string;
  containers: Container[];
}

export interface Activity {
  id: string;
  projectId: string;
  time: string;
  action: string;
  result: "success" | "failed" | "running";
  duration: string;
}

export type TerminalLineType =
  | "command"
  | "info"
  | "success"
  | "error"
  | "muted";

export interface TerminalLine {
  id: string;
  time: string;
  type: TerminalLineType;
  text: string;
}

export interface Backup {
  name: string;
  size: string;
  createdAt: string;
}

export const servers: Server[] = [
  {
    id: "prod",
    name: "生产环境",
    host: "47.106.226.129",
    environment: "production",
    status: "connected",
    latency: "42ms",
  },
  {
    id: "staging",
    name: "预发布",
    host: "192.168.56.120",
    environment: "staging",
    status: "connected",
    latency: "8ms",
  },
  {
    id: "local",
    name: "本地 vbox",
    host: "filesystem",
    environment: "local",
    status: "connected",
    latency: "本地",
  },
];

export const projects: Project[] = [
  {
    id: "boyu",
    serverId: "prod",
    name: "boyu",
    title: "博与科技",
    path: "/data/phalcon-admin",
    domain: "gu19.cn",
    port: 8071,
    branch: "main",
    status: "running",
    ssl: true,
    lastDeploy: "12 分钟前",
    backups: 3,
    phpVersion: "PHP 8.5.12",
    containers: [
      { service: "nginx", image: "nginx:stable-alpine", status: "running", port: "8071" },
      { service: "php", image: "authus/phalcon:5.23.0", status: "running", port: "-" },
      { service: "mysql", image: "mysql:8.1.0", status: "running", port: "-" },
      { service: "redis", image: "redis:7.2-alpine", status: "running", port: "-" },
    ],
  },
  {
    id: "yihe",
    serverId: "prod",
    name: "yihe",
    title: "宜和业务",
    path: "/data/yihe",
    domain: "yihe.example.com",
    port: 8072,
    branch: "main",
    status: "running",
    ssl: true,
    lastDeploy: "2 小时前",
    backups: 2,
    phpVersion: "PHP 8.5.12",
    containers: [
      { service: "nginx", image: "nginx:stable-alpine", status: "running", port: "8072" },
      { service: "php", image: "authus/phalcon:5.23.0", status: "running", port: "-" },
      { service: "mysql", image: "mysql:8.1.0", status: "running", port: "-" },
      { service: "redis", image: "redis:7.2-alpine", status: "running", port: "-" },
    ],
  },
  {
    id: "boyu-staging",
    serverId: "staging",
    name: "boyu-staging",
    title: "博与科技 · 预发布",
    path: "/data/staging/phalcon-admin",
    domain: "staging.gu19.cn",
    port: 8073,
    branch: "develop",
    status: "warning",
    ssl: false,
    lastDeploy: "昨天",
    backups: 1,
    phpVersion: "PHP 8.5.12",
    containers: [
      { service: "nginx", image: "nginx:stable-alpine", status: "running", port: "8073" },
      { service: "php", image: "authus/phalcon:5.23.0", status: "running", port: "-" },
      { service: "mysql", image: "mysql:8.1.0", status: "stopped", port: "-" },
      { service: "redis", image: "redis:7.2-alpine", status: "running", port: "-" },
    ],
  },
  {
    id: "vbox",
    serverId: "local",
    name: "vbox",
    title: "本地 vbox",
    path: "D:/demo/code/phpProjects/vbox",
    domain: "vbox.local",
    port: 8074,
    branch: "main",
    status: "running",
    ssl: false,
    lastDeploy: "5 分钟前",
    backups: 0,
    phpVersion: "PHP 8.5.12",
    containers: [
      { service: "nginx", image: "nginx:stable-alpine", status: "running", port: "8074" },
      { service: "php", image: "authus/phalcon:5.23.0", status: "running", port: "-" },
      { service: "mysql", image: "mysql:8.1.0", status: "running", port: "-" },
      { service: "redis", image: "redis:7.2-alpine", status: "running", port: "-" },
    ],
  },
  {
    id: "vbox2",
    serverId: "local",
    name: "vbox2",
    title: "vbox2 · 历史测试",
    path: "D:/demo/code/phpProjects/vbox2",
    domain: "vbox2.local",
    port: 8075,
    branch: "main",
    status: "not-deployed",
    ssl: false,
    lastDeploy: "从未",
    backups: 0,
    phpVersion: "PHP 8.5.12",
    containers: [],
  },
];

export const activities: Activity[] = [
  { id: "a1", projectId: "boyu", time: "12:30", action: "upgrade", result: "success", duration: "2m 18s" },
  { id: "a2", projectId: "boyu", time: "11:05", action: "db:backup", result: "success", duration: "18.4s" },
  { id: "a3", projectId: "boyu", time: "昨天 22:14", action: "dc:restart:php", result: "success", duration: "3.2s" },
  { id: "a4", projectId: "yihe", time: "10:12", action: "upgrade", result: "success", duration: "1m 42s" },
  { id: "a5", projectId: "yihe", time: "昨天 16:40", action: "db:backup", result: "success", duration: "12.1s" },
  { id: "a6", projectId: "boyu-staging", time: "昨天 18:20", action: "upgrade", result: "failed", duration: "48s" },
  { id: "a7", projectId: "vbox", time: "5 分钟前", action: "init", result: "success", duration: "3m 06s" },
  { id: "a7b", projectId: "vbox", time: "5 分钟前", action: "dc:status", result: "success", duration: "0.8s" },
  { id: "a7c", projectId: "vbox", time: "4 分钟前", action: "db:backup", result: "success", duration: "1.2s" },
  { id: "a7d", projectId: "vbox", time: "3 分钟前", action: "nginx:reload", result: "success", duration: "0.6s" },
  { id: "a8", projectId: "boyu", time: "昨天 20:10", action: "db:migrate", result: "success", duration: "12.4s" },
  { id: "a9", projectId: "boyu", time: "昨天 18:02", action: "nginx:reload", result: "success", duration: "1.8s" },
];

export const backups: Backup[] = [
  { name: "phalcon-admin_2026-10-24_030001.sql.gz", size: "18.4 MB", createdAt: "今天 03:00" },
  { name: "phalcon-admin_2026-10-23_030001.sql.gz", size: "18.2 MB", createdAt: "昨天 03:00" },
  { name: "phalcon-admin_2026-10-22_030001.sql.gz", size: "17.9 MB", createdAt: "10-22 03:00" },
];

export const initialTerminalLines: Record<string, TerminalLine[]> = {
  boyu: [
    { id: "b1", time: "12:30:01", type: "command", text: "开始部署：同步全部 + 上传配置" },
    { id: "b2", time: "12:30:02", type: "info", text: "→ [1/2] [bundle] 主仓库" },
    { id: "b3", time: "12:30:46", type: "info", text: "→ [2/2] [ftp] src/App/Projects/boyu" },
    { id: "b4", time: "12:31:18", type: "success", text: "✔ 代码同步完成（2m 18s）" },
    { id: "b5", time: "12:31:19", type: "muted", text: "提示：更新代码后请重启 php 容器" },
  ],
  yihe: [
    { id: "y1", time: "10:12:03", type: "command", text: "开始部署：同步全部 + 上传配置" },
    { id: "y2", time: "10:12:04", type: "info", text: "→ 同步主仓库 main ..." },
    { id: "y3", time: "10:13:41", type: "success", text: "✔ 部署完成（1m 42s）" },
  ],
  vbox: [
    { id: "v1", time: "22:02:10", type: "command", text: "本地生成配置（filesystem 目标）" },
    { id: "v2", time: "22:02:11", type: "info", text: "→ filesystem 目标，仅同步本地目录" },
    { id: "v3", time: "22:05:16", type: "success", text: "✔ 本地配置生成完成" },
  ],
};
