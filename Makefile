CXX = clang++
#CXX = g++
CXXFLAG_1= -I./include -I/prog/vcpkg/installed/x64-windows/include
CXXFLAG_LIB_1= -L/prog/vcpkg/installed/x64-windows/lib

CXXFLAGS = -shared -fPIC -std=c++17 -lsqlite3 -lcurl -luuid $(CXXFLAG_1) $(CXXFLAG_LIB_1)
#CXXFLAGS = -std=c++17 $(CXXFLAG_1) $(CXXFLAG_2)
#g++ -shared -fPIC -o libsample.so sample.cpp

TARGET = libsample.so
all: $(TARGET)

$(TARGET): sample.o
	$(CXX) $(CXXFLAGS) sample.o -o $(TARGET)

sample.o: sample.cpp
	$(CXX) $(CXXFLAGS) -c sample.cpp

clean:
	rm *.o $(TARGET)
