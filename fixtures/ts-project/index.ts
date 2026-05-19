interface Greetable {
  greet(): string;
}

class User implements Greetable {
  constructor(public name: string, public age: number) {}

  greet(): string {
    return `Hello, ${this.name}!`;
  }
}

class Admin implements Greetable {
  constructor(public user: User) {}

  greet(): string {
    return `Admin: ${this.user.name}`;
  }
}

function greetEntity(entity: Greetable): string {
  return entity.greet();
}

const user = new User("Alice", 30);
greetEntity(user);

const admin = new Admin(user);
greetEntity(admin);
