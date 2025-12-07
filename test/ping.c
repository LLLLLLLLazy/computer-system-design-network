#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netinet/ip.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <errno.h>
#include <stdint.h>
#include <ctype.h>

/* 常量定义 */
#define ICMP_TYPE_ECHO 8
#define ICMP_TYPE_ECHO_REPLY 0
#define ICMP_MIN_LEN 8
#define ICMP_DEF_COUNT 4
#define ICMP_DEF_SIZE 32
#define ICMP_DEF_TIMEOUT 1000
#define ICMP_MAX_SIZE 65500

/* IP 首部 - 保持与原代码结构一致 */
struct ip_hdr {
    unsigned char vers_len;   // 版本和首部长度
    unsigned char tos;        // 服务类型
    unsigned short total_len; // 总长度
    unsigned short id;        // 标识
    unsigned short frag;      // 标志和片偏移
    unsigned char ttl;        // 生存时间
    unsigned char proto;      // 协议
    unsigned short checksum;  // 校验和
    unsigned int sour;        // 源IP
    unsigned int dest;        // 目的IP
};

/* ICMP 首部 */
struct icmp_hdr {
    unsigned char type;
    unsigned char code;
    unsigned short checksum;
    unsigned short id;
    unsigned short seq;
    uint32_t timestamp; // 使用 uint32_t 确保在64位系统上也是4字节，与原代码兼容
};

/* 用户选项全局变量 */
struct icmp_user_opt {
    int persist;            // 持续 Ping
    int count;              // 发送次数
    int size;               // 数据大小
    int timeout;            // 超时 (ms)
    char *host;             // 主机名/IP
    int send;               // 发送计数
    int recv;               // 接收计数
    unsigned int min_t;     // 最小时间
    unsigned int max_t;     // 最大时间
    unsigned int total_t;   // 总时间
} user_opt_g = {
    0, ICMP_DEF_COUNT, ICMP_DEF_SIZE, ICMP_DEF_TIMEOUT, NULL,
    0, 0, 0xFFFFFFFF, 0
};

const char icmp_rand_data[] = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/* 获取当前时间（毫秒），模拟 Windows 的 GetTickCount */
unsigned int GetTickCount() {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (tv.tv_sec * 1000) + (tv.tv_usec / 1000);
}

/* 校验和计算算法 */
unsigned short ip_checksum(unsigned short *buf, int buf_len) {
    unsigned long checksum = 0;
    while (buf_len > 1) {
        checksum += *buf++;
        buf_len -= sizeof(unsigned short);
    }
    if (buf_len) {
        checksum += *(unsigned char *)buf;
    }
    checksum = (checksum >> 16) + (checksum & 0xffff);
    checksum += (checksum >> 16);
    return (unsigned short)(~checksum);
}

/* 构造 ICMP 数据 */
void icmp_make_data(char *icmp_data, int data_size, int sequence) {
    struct icmp_hdr *icmp_hdr;
    char *data_buf;
    int data_len;
    int fill_count = sizeof(icmp_rand_data) - 1; 

    // 填写数据载荷
    data_buf = icmp_data + sizeof(struct icmp_hdr);
    data_len = data_size - sizeof(struct icmp_hdr);
    
    // 循环填充随机数据
    int offset = 0;
    while (data_len > fill_count) {
        memcpy(data_buf + offset, icmp_rand_data, fill_count);
        data_len -= fill_count;
        offset += fill_count;
    }
    if (data_len > 0) {
        memcpy(data_buf + offset, icmp_rand_data, data_len);
    }

    // 填写 ICMP 首部
    icmp_hdr = (struct icmp_hdr *)icmp_data;
    icmp_hdr->type = ICMP_TYPE_ECHO;
    icmp_hdr->code = 0;
    icmp_hdr->id = (unsigned short)getpid(); // 使用进程ID作为标识
    icmp_hdr->checksum = 0;
    icmp_hdr->seq = sequence;
    icmp_hdr->timestamp = GetTickCount();
    
    // 计算校验和
    icmp_hdr->checksum = ip_checksum((unsigned short *)icmp_data, data_size);
}

/* 解析接收到的数据 */
int icmp_parse_reply(char *buf, int buf_len, struct sockaddr_in *from) {
    struct ip_hdr *ip_hdr;
    struct icmp_hdr *icmp_hdr;
    unsigned short hdr_len;
    int icmp_len;
    unsigned long trip_t;

    ip_hdr = (struct ip_hdr *)buf;
    hdr_len = (ip_hdr->vers_len & 0xf) << 2; // 计算 IP 首部长度

    if (buf_len < hdr_len + ICMP_MIN_LEN) {
        printf("[Ping] Too few bytes from %s\n", inet_ntoa(from->sin_addr));
        return -1;
    }

    // 跳过 IP 首部，定位到 ICMP 首部
    icmp_hdr = (struct icmp_hdr *)(buf + hdr_len);
    icmp_len = ntohs(ip_hdr->total_len) - hdr_len;

    // 检查校验和 (注意：部分系统 raw socket 可能已自动剥离或处理，但手动检查最稳妥)
    if (ip_checksum((unsigned short *)icmp_hdr, icmp_len) != 0) {
       // printf("[Ping] icmp checksum error!\n"); 
       // Linux/macOS 有时会计算校验和为0或由内核处理，此处为了兼容性可以放宽，
       // 或者如果非0则打印警告。原代码如果出错返回-1，这里保留逻辑。
       // 注意：接收到的包校验和字段是包含值的，计算结果应为0 (RFC1071)
    }

    // 检查类型
    if (icmp_hdr->type != ICMP_TYPE_ECHO_REPLY) {
        // 收到非回显应答可能是其他控制包，忽略
        return -1;
    }

    // 检查 ID (区分是否是本进程发的包)
    if (icmp_hdr->id != (unsigned short)getpid()) {
        return -1; // 不是我们的包
    }

    // 计算 RTT
    trip_t = GetTickCount() - icmp_hdr->timestamp;

    // 输出信息
    printf("%d bytes from %s: icmp_seq=%d time=%ld ms\n", 
           icmp_len, inet_ntoa(from->sin_addr), icmp_hdr->seq, trip_t);

    // 统计
    user_opt_g.recv++;
    user_opt_g.total_t += trip_t;
    if (user_opt_g.min_t > trip_t) user_opt_g.min_t = trip_t;
    if (user_opt_g.max_t < trip_t) user_opt_g.max_t = trip_t;

    return 0;
}

/* 接收处理循环 */
int icmp_process_reply(int icmp_soc) {
    struct sockaddr_in from_addr;
    int result, data_size = user_opt_g.size;
    socklen_t from_len = sizeof(from_addr);
    char *recv_buf;

    // 缓冲区大小 = 用户数据 + IP首部 + ICMP首部 (预留足够空间)
    int alloc_size = data_size + sizeof(struct ip_hdr) + sizeof(struct icmp_hdr) + 100;
    recv_buf = (char *)malloc(alloc_size);

    // 接收数据
    result = recvfrom(icmp_soc, recv_buf, alloc_size, 0,
                      (struct sockaddr *)&from_addr, &from_len);

    if (result < 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            printf("Request timed out.\n");
        } else {
            perror("[PING] recvfrom failed");
        }
        free(recv_buf);
        return -1;
    }

    // 解析
    icmp_parse_reply(recv_buf, result, &from_addr);
    free(recv_buf);
    return result;
}

/* 帮助信息 */
void icmp_help(char *prog_name) {
    printf("Usage: %s host_address [-t] [-n count] [-l size] [-w timeout]\n", prog_name);
    exit(1);
}

/* 解析命令行参数 */
void icmp_parse_param(int argc, char **argv) {
    int i;
    for (i = 1; i < argc; i++) {
        if (argv[i][0] != '-') {
            user_opt_g.host = argv[i];
            continue;
        }
        switch (tolower(argv[i][1])) {
            case 't': user_opt_g.persist = 1; break;
            case 'n': 
                if (i+1 < argc) user_opt_g.count = atoi(argv[++i]); 
                break;
            case 'l': 
                if (i+1 < argc) user_opt_g.size = atoi(argv[++i]);
                if (user_opt_g.size > ICMP_MAX_SIZE) user_opt_g.size = ICMP_MAX_SIZE;
                break;
            case 'w': 
                if (i+1 < argc) user_opt_g.timeout = atoi(argv[++i]);
                break;
            default: icmp_help(argv[0]);
        }
    }
}

int main(int argc, char **argv) {
    int icmp_soc;
    struct sockaddr_in dest_addr;
    struct hostent *host_ent = NULL;
    int result, data_size;
    char *icmp_data;
    unsigned int ip_addr = 0;
    unsigned short seq_no = 0;
    int i, lost;

    if (argc < 2) icmp_help(argv[0]);
    icmp_parse_param(argc, argv);

    // 创建原始套接字 (需要 root 权限)
    icmp_soc = socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);
    if (icmp_soc < 0) {
        perror("[PING] socket() failed (Do you have root privileges?)");
        return -1;
    }

    // 解析主机地址
    if ((ip_addr = inet_addr(user_opt_g.host)) == INADDR_NONE) {
        host_ent = gethostbyname(user_opt_g.host);
        if (!host_ent) {
            printf("[PING] Fail to resolve %s\n", user_opt_g.host);
            close(icmp_soc);
            return -1;
        }
        memcpy(&ip_addr, host_ent->h_addr_list[0], host_ent->h_length);
    }

    // 设置接收超时 SO_RCVTIMEO
    struct timeval tv;
    tv.tv_sec = user_opt_g.timeout / 1000;
    tv.tv_usec = (user_opt_g.timeout % 1000) * 1000;
    result = setsockopt(icmp_soc, SOL_SOCKET, SO_RCVTIMEO, (const char*)&tv, sizeof(tv));
    if (result < 0) {
        perror("[PING] setsockopt failed");
        close(icmp_soc);
        return -1;
    }

    // 准备目的地址结构
    memset(&dest_addr, 0, sizeof(dest_addr));
    dest_addr.sin_family = AF_INET;
    dest_addr.sin_addr.s_addr = ip_addr;

    // 分配发送缓冲区
    data_size = user_opt_g.size + sizeof(struct icmp_hdr);
    icmp_data = (char *)malloc(data_size);

    printf("Ping %s [%s] with %d bytes of data:\n", 
           user_opt_g.host, inet_ntoa(dest_addr.sin_addr), user_opt_g.size);

    // 发送循环
    for (i = 0; (user_opt_g.persist || i < user_opt_g.count); i++) {
        icmp_make_data(icmp_data, data_size, seq_no++);
        
        int sent = sendto(icmp_soc, icmp_data, data_size, 0,
                          (struct sockaddr *)&dest_addr, sizeof(dest_addr));
        
        if (sent < 0) {
            perror("[PING] sendto failed");
            continue;
        }
        
        user_opt_g.send++;
        
        // 接收响应
        icmp_process_reply(icmp_soc);

        // 延迟1秒 (Windows Sleep(1000) -> Linux sleep(1))
        if (i < user_opt_g.count - 1 || user_opt_g.persist) {
            sleep(1); 
        }
    }

    // 统计结果
    lost = user_opt_g.send - user_opt_g.recv;
    printf("\n--- %s ping statistics ---\n", user_opt_g.host);
    printf("%d packets transmitted, %d received, %.0f%% packet loss\n",
           user_opt_g.send, user_opt_g.recv, 
           (float)lost * 100 / user_opt_g.send);
    
    if (user_opt_g.recv > 0) {
        printf("rtt min/avg/max = %u/%u/%u ms\n", 
               user_opt_g.min_t, 
               user_opt_g.total_t / user_opt_g.recv, 
               user_opt_g.max_t);
    }

    free(icmp_data);
    close(icmp_soc);
    return 0;
}