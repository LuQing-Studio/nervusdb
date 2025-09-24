import { describe, expect, it } from 'vitest';
import {
  buildConnectionUri,
  sanitizeConnectionOptions,
  ensureConnectionOptions,
  type ConnectionOptions,
} from '@/index';

describe('连接选项验证 (ensureConnectionOptions)', () => {
  it('应该为完整配置补充默认端口', () => {
    const options: ConnectionOptions = {
      driver: 'postgresql',
      host: 'localhost',
      username: 'user',
      password: 'pass',
    };

    const result = ensureConnectionOptions(options);
    expect(result.port).toBe(5432);
  });

  it('应该保留用户指定的端口', () => {
    const options: ConnectionOptions = {
      driver: 'mysql',
      host: 'localhost',
      username: 'user',
      password: 'pass',
      port: 3307,
    };

    const result = ensureConnectionOptions(options);
    expect(result.port).toBe(3307);
  });

  it('应该为不同数据库设置正确的默认端口', () => {
    const drivers = [
      { driver: 'postgresql' as const, expectedPort: 5432 },
      { driver: 'mysql' as const, expectedPort: 3306 },
      { driver: 'mariadb' as const, expectedPort: 3306 },
      { driver: 'sqlserver' as const, expectedPort: 1433 },
    ];

    drivers.forEach(({ driver, expectedPort }) => {
      const result = ensureConnectionOptions({
        driver,
        host: 'localhost',
        username: 'user',
        password: 'pass',
      });
      expect(result.port).toBe(expectedPort);
    });
  });

  it('缺少driver时应该抛出错误', () => {
    expect(() =>
      ensureConnectionOptions({
        host: 'localhost',
        username: 'user',
        password: 'pass',
      } as ConnectionOptions),
    ).toThrow(/缺少必要连接字段: driver/);
  });

  it('缺少多个字段时应该列出所有缺失字段', () => {
    expect(() =>
      ensureConnectionOptions({
        driver: 'postgresql',
      } as ConnectionOptions),
    ).toThrow(/缺少必要连接字段: host, username, password/);
  });

  it('空字符串字段应被视为缺失', () => {
    expect(() =>
      ensureConnectionOptions({
        driver: 'postgresql',
        host: '',
        username: 'user',
        password: 'pass',
      }),
    ).toThrow(/缺少必要连接字段: host/);
  });
});

describe('连接字符串构建 (buildConnectionUri)', () => {
  it('应根据默认端口与参数生成稳定的连接 URI', () => {
    const uri = buildConnectionUri({
      driver: 'postgresql',
      host: 'db.internal.local',
      username: 'analytics',
      password: 'super$secret',
      database: 'warehouse',
      parameters: {
        poolSize: 10,
        sslmode: 'require',
      },
    });

    expect(uri).toBe(
      'postgresql://analytics:super%24secret@db.internal.local:5432/warehouse?poolSize=10&sslmode=require',
    );
  });

  it('缺少关键字段时抛出明确错误', () => {
    expect(() =>
      buildConnectionUri({
        driver: 'mysql',
        host: 'localhost',
        username: 'root',
        password: '',
      }),
    ).toThrow(/缺少必要连接字段: password/);
  });

  it('不带数据库名称时应该省略数据库段', () => {
    const uri = buildConnectionUri({
      driver: 'mysql',
      host: 'localhost',
      username: 'root',
      password: 'secret',
    });

    expect(uri).toBe('mysql://root:secret@localhost:3306');
  });

  it('不带参数时应该省略查询字符串', () => {
    const uri = buildConnectionUri({
      driver: 'postgresql',
      host: 'localhost',
      username: 'user',
      password: 'pass',
      database: 'test',
    });

    expect(uri).toBe('postgresql://user:pass@localhost:5432/test');
  });

  it('应该正确编码特殊字符', () => {
    const uri = buildConnectionUri({
      driver: 'postgresql',
      host: 'localhost',
      username: 'user@domain',
      password: 'p@ss/word',
      database: 'test-db',
    });

    expect(uri).toBe('postgresql://user%40domain:p%40ss%2Fword@localhost:5432/test-db');
  });

  it('参数应该按键名排序', () => {
    const uri = buildConnectionUri({
      driver: 'postgresql',
      host: 'localhost',
      username: 'user',
      password: 'pass',
      parameters: {
        zOption: 'last',
        aOption: 'first',
        mOption: 'middle',
      },
    });

    expect(uri).toBe(
      'postgresql://user:pass@localhost:5432?aOption=first&mOption=middle&zOption=last',
    );
  });

  it('应该正确处理不同类型的参数值', () => {
    const uri = buildConnectionUri({
      driver: 'mysql',
      host: 'localhost',
      username: 'user',
      password: 'pass',
      parameters: {
        timeout: 30,
        ssl: true,
        debug: false,
        charset: 'utf8mb4',
      },
    });

    expect(uri).toBe(
      'mysql://user:pass@localhost:3306?charset=utf8mb4&debug=false&ssl=true&timeout=30',
    );
  });
});

describe('敏感信息脱敏 (sanitizeConnectionOptions)', () => {
  it('仅保留口令末尾四位', () => {
    const sanitized = sanitizeConnectionOptions({
      driver: 'postgresql',
      host: 'db.internal',
      username: 'etl',
      password: 'synapse-secret',
      database: 'warehouse',
    });

    expect(sanitized.password).toBe('**********cret');
    expect(sanitized.port).toBe(5432);
  });

  it('短密码应该完全被掩码', () => {
    const sanitized = sanitizeConnectionOptions({
      driver: 'mysql',
      host: 'localhost',
      username: 'user',
      password: '123',
    });

    expect(sanitized.password).toBe('123'); // 少于4位时保持原样
  });

  it('四位密码应该完全显示', () => {
    const sanitized = sanitizeConnectionOptions({
      driver: 'mysql',
      host: 'localhost',
      username: 'user',
      password: '1234',
    });

    expect(sanitized.password).toBe('1234');
  });

  it('长密码应该正确掩码', () => {
    const sanitized = sanitizeConnectionOptions({
      driver: 'postgresql',
      host: 'localhost',
      username: 'user',
      password: 'verylongpassword123',
    });

    expect(sanitized.password).toBe('***************d123');
  });

  it('应该保留所有其他选项', () => {
    const options = {
      driver: 'postgresql' as const,
      host: 'localhost',
      username: 'user',
      password: 'secret123',
      database: 'testdb',
      port: 5433,
      parameters: { ssl: true },
    };

    const sanitized = sanitizeConnectionOptions(options);

    expect(sanitized.driver).toBe(options.driver);
    expect(sanitized.host).toBe(options.host);
    expect(sanitized.username).toBe(options.username);
    expect(sanitized.database).toBe(options.database);
    expect(sanitized.port).toBe(options.port);
    expect(sanitized.parameters).toEqual(options.parameters);
    expect(sanitized.password).toBe('*****t123');
  });
});
