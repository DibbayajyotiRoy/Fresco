"use client";

import * as React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Bell,
  ChatCircle,
  SquaresFour,
  Waves,
} from "@phosphor-icons/react/dist/ssr";

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar";

const NAV = [
  { title: "Overview", href: "/", icon: SquaresFour },
  { title: "Notifications", href: "/notifications", icon: Bell },
  { title: "Feedback", href: "/feedback", icon: ChatCircle },
] as const;

export function AppSidebar() {
  const pathname = usePathname();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <Link href="/">
                <div className="from-brand text-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg bg-gradient-to-br to-sky-500 shadow-[inset_0_1px_0_rgba(255,255,255,0.25)]">
                  <Waves className="size-4" weight="duotone" />
                </div>
                <div className="grid flex-1 text-left leading-tight">
                  <span className="truncate font-serif text-base font-medium tracking-tight">
                    Fresco Admin
                  </span>
                  <span className="text-muted-foreground truncate text-xs">
                    Live wallpaper
                  </span>
                </div>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Dashboard</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV.map((item) => {
                const active =
                  item.href === "/"
                    ? pathname === "/"
                    : pathname.startsWith(item.href);
                return (
                  <SidebarMenuItem key={item.href}>
                    <SidebarMenuButton
                      asChild
                      isActive={active}
                      tooltip={item.title}
                    >
                      <Link href={item.href}>
                        <item.icon weight="duotone" />
                        <span>{item.title}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <div className="text-muted-foreground px-2 py-1 text-xs group-data-[collapsible=icon]:hidden">
          fresco · admin
        </div>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
