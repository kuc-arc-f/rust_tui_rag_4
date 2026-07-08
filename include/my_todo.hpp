#pragma once
#include <iostream>
#include <fstream>
#include <vector>
#include <string>
#include "nlohmann/json.hpp"

#include "my_db.hpp"

using json = nlohmann::json;

const std::string DB_PATH = "todo.db";

struct TodoData {
    int max_id;
    std::vector<Todo> items;
};

class MyTodo {
private:
    std::string m_name;

    public:
    explicit MyTodo(std::string str){}

    ~MyTodo() {}
    
    void todo_add_handler(std::string input_str){
        try{
            MyDb db_helper(DB_PATH);
            db_helper.add(input_str);
        } catch (const std::exception& e) {
            std::cout << "Error , main" << std::endl;
        }  
    }

    std::string todo_list_handler(){
        std::string ret;
        try{
            MyDb db_helper(DB_PATH);
            auto todos = db_helper.list("all");
            ret = db_helper.list_json(todos);
            return ret;
        } catch (const std::exception& e) {
            std::cout << "Error , main" << std::endl;
            return ret;
        }  
    }

    void todo_delete_handler(int id){
        try{
            MyDb db_helper(DB_PATH);
            db_helper.remove(id);
        } catch (const std::exception& e) {
            std::cout << "Error , main" << std::endl;
        }  
    }

};
