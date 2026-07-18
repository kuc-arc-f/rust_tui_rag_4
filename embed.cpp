
#include <fstream>
#include <filesystem>
#include <iostream>
#include <string>
#include <sstream>
#include <map>
#include <stdexcept>
#include <vector>
#include <curl/curl.h>
#include <nlohmann/json.hpp>
#include <dotenv.h>

#include "db_add.hpp"
#include "my_config.hpp"
#include "include/GeminiEmbeddingClient.hpp"

using namespace std;

using json = nlohmann::json;

const std::string DB_PATH = "example.db";

// ─────────────────────────────────────────────
// レスポンス構造体
// ─────────────────────────────────────────────
struct HttpResponse {
    long        status_code = 0;
    std::string body;
    std::string error;

    bool is_ok() const { return status_code >= 200 && status_code < 300; }
};

// ─────────────────────────────────────────────
// libcurl 書き込みコールバック
// ─────────────────────────────────────────────
static size_t write_callback(char* ptr, size_t size, size_t nmemb, void* userdata)
{
    size_t total = size * nmemb;
    std::string* body = static_cast<std::string*>(userdata);
    body->append(ptr, total);
    return total;
}

// ─────────────────────────────────────────────
// HttpClient クラス
// ─────────────────────────────────────────────
class HttpClient {
public:
    using Headers = std::map<std::string, std::string>;

    // タイムアウト(秒)・SSL検証はコンストラクタで設定可能
    explicit HttpClient(long timeout_sec = 30, bool verify_ssl = true)
        : timeout_sec_(timeout_sec), verify_ssl_(verify_ssl)
    {
        curl_global_init(CURL_GLOBAL_DEFAULT);
    }

    ~HttpClient()
    {
        curl_global_cleanup();
    }

    // ── GET ──────────────────────────────────
    HttpResponse get(const std::string& url,
                     const Headers& headers = {}) const
    {
        return perform(url, "GET", "", headers);
    }

    // ── POST ─────────────────────────────────
    HttpResponse post(const std::string& url,
                      const std::string& body,
                      const Headers& headers = {}) const
    {
        return perform(url, "POST", body, headers);
    }

    // ── POST (JSON 簡易ラッパー) ──────────────
    HttpResponse post_json(const std::string& url,
                           const std::string& json_body,
                           Headers headers = {}) const
    {
        headers["Content-Type"] = "application/json";
        return post(url, json_body, headers);
    }

private:
    long timeout_sec_;
    bool verify_ssl_;

    HttpResponse perform(const std::string& url,
                         const std::string& method,
                         const std::string& body,
                         const Headers& headers) const
    {
        HttpResponse response;
        CURL* curl = curl_easy_init();
        if (!curl) {
            response.error = "curl_easy_init() failed";
            return response;
        }

        // ── ヘッダー設定 ──────────────────────
        struct curl_slist* header_list = nullptr;
        for (const auto& [key, val] : headers) {
            std::string header_str = key + ": " + val;
            header_list = curl_slist_append(header_list, header_str.c_str());
        }

        // ── 基本オプション ────────────────────
        curl_easy_setopt(curl, CURLOPT_URL,            url.c_str());
        curl_easy_setopt(curl, CURLOPT_TIMEOUT,        timeout_sec_);
        curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT, 10L);
        curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);   // リダイレクト追従
        curl_easy_setopt(curl, CURLOPT_MAXREDIRS,      5L);
        curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, verify_ssl_ ? 1L : 0L);
        curl_easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, verify_ssl_ ? 2L : 0L);
        curl_easy_setopt(curl, CURLOPT_USERAGENT,      "HttpClient/1.0");

        // ── レスポンスボディ受信 ───────────────
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA,     &response.body);

        // ── メソッド別設定 ────────────────────
        if (method == "POST") {
            curl_easy_setopt(curl, CURLOPT_POST,           1L);
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS,     body.c_str());
            curl_easy_setopt(curl, CURLOPT_POSTFIELDSIZE,
                             static_cast<long>(body.size()));
        } else {
            // GET (デフォルト)
            curl_easy_setopt(curl, CURLOPT_HTTPGET, 1L);
        }

        if (header_list) {
            curl_easy_setopt(curl, CURLOPT_HTTPHEADER, header_list);
        }

        // ── 実行 ──────────────────────────────
        CURLcode res = curl_easy_perform(curl);
        if (res != CURLE_OK) {
            response.error = curl_easy_strerror(res);
        } else {
            curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &response.status_code);
        }

        // ── クリーンアップ ────────────────────
        curl_slist_free_all(header_list);
        curl_easy_cleanup(curl);

        return response;
    }
};

// ─────────────────────────────────────────────
// ユーティリティ：レスポンス表示
// ─────────────────────────────────────────────
static void print_response(const std::string& label, const HttpResponse& resp)
{
    std::cout << "\n===== " << label << " =====\n";
    if (!resp.error.empty()) {
        std::cerr << "[ERROR] " << resp.error << "\n";
        return;
    }
    std::cout << "Status : " << resp.status_code << "\n";
    std::cout << "Body   :\n" << resp.body << "\n";
}

struct QueryReq {
    std::string input;
};   
NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE(QueryReq, input)

/**
*
* @param
*
* @return
*/
int ebmed(std::string query){
        
    try{
        int ret = 0;
        auto embeddings = EmbeddingStart(query);
        std::cout << "vlen=" << embeddings.size() << std::endl;
        auto vec = embeddings;
        DbAdd app(DB_PATH);
        //std::vector<float> tmp_vec = {0.1f, 0.2f, 0.3f};
        app.add_embed(vec, query);
    } catch (const std::exception& e) {
        std::cout << "Error , main" << std::endl;
        return 1;
    }        
    return 0;
}

// 1ファイル分のデータを保持する構造体
struct TextFile {
    std::string filename;
    std::vector<std::string> lines;
};

// .txt ファイルを読み込んで行を返す
TextFile loadTextFile(const std::filesystem::path& filepath) {
    TextFile tf;
    tf.filename = filepath.filename().string();

    std::ifstream ifs(filepath);
    if (!ifs.is_open()) {
        std::cerr << "[警告] ファイルを開けません: " << filepath << "\n";
        return tf;
    }

    std::string line;
    while (std::getline(ifs, line)) {
        tf.lines.push_back(line);
    }
    return tf;
}

// 読み込んだデータを表示する
void addTextFiles(const std::vector<TextFile>& files) {
    for (const auto& tf : files) {
        std::cout << "========================================\n";
        std::cout << "ファイル名: " << tf.filename << "\n";
        std::cout << "行数      : " << tf.lines.size() << "\n";
        std::cout << "----------------------------------------\n";
        std:string target = "";
        for (size_t i = 0; i < tf.lines.size(); ++i) {
            //std::cout << "[" << i + 1 << "] " << tf.lines[i] << "\n";
            std::string tmp = tf.lines[i] + "\n";
            target.append(tmp);
        }
        std::cout <<  target << "\n";
        int resp = ebmed(target);
        std::cout << "resp=" << resp << "\n";
    }
    std::cout << "========================================\n";
}
/**
*
* @param
*
* @return
*/
int main(int argc, char* argv[])
{
    dotenv::init();
    const char* api_key = std::getenv("GEMINI_API_KEY");
    if (api_key != nullptr) {
        std::cout << "api_key:" << api_key << std::endl;
    }else{
        std::cerr << "Error: GEMINI_API_KEY environment variable not set" << std::endl;
        std::cerr << "Please set it with: export GEMINI_API_KEY=your_api_key_here" << std::endl;
        return -1;
    }

    std::cout << "  DB: " << DB_PATH << "\n\n";

    // 引数でフォルダを指定、省略時はカレントディレクトリ
    std::string dirPath = (argc >= 2) ? argv[1] : ".";
    if(argc < 2) {
        std::cerr << "[ERROR] argment none" << "\n";
        return 0;
    }    

    if (!std::filesystem::exists(dirPath) || !std::filesystem::is_directory(dirPath)) {
        std::cerr << "[エラー] 有効なディレクトリではありません: " << dirPath << "\n";
        return 1;
    }

    std::cout << "対象フォルダ: " << std::filesystem::absolute(dirPath) << "\n\n";

    std::vector<TextFile> allFiles;

    // フォルダ内の .txt ファイルをすべて列挙
    for (const auto& entry : std::filesystem::directory_iterator(dirPath)) {
        if (entry.is_regular_file() && 
        (entry.path().extension() == ".txt" || entry.path().extension() == ".md") ) {
            TextFile tf = loadTextFile(entry.path());
            allFiles.push_back(std::move(tf));
        }
    }
    if (allFiles.empty()) {
        std::cout << ".txt ファイルが見つかりませんでした。\n";
        return 0;
    }
    std::cout << "読み込んだファイル数: " << allFiles.size() << "\n\n";
    addTextFiles(allFiles);

    return 0;
}

