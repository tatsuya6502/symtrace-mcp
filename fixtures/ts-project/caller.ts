import { greetEntity, User } from "./index";

export function useGreeting(): string {
  return greetEntity(new User("Bob", 25));
}
